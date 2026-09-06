//! Resolve registered resource links before a production media write.
use super::*;
use dsf_r2_actions::ApplyConfiguration as Bucket;
use dsf_railway_actions::Deploy as Api;

fn target<A: ResourceAction>(
    runtime: &mut Runtime<impl Host>,
    id: &str,
) -> Result<A::Target, Error> {
    let row = runtime.row(A::ENTITY_SET, id)?;
    let raw = runtime.read("Files", required(&row, "config_ref")?, true)?;
    if raw.status != 200 || raw.body.len() > 32768 {
        return Err(Error::Response("linked resource configuration"));
    }
    if format!("{:x}", Sha256::digest(raw.body.as_bytes())) != required(&row, "config_sha256")? {
        return Err(Error::Binding("linked resource configuration hash differs"));
    }
    let config: ResourceConfig<A::Target> = serde_json::from_str(&raw.body)
        .map_err(|_| Error::Binding("invalid linked resource configuration"))?;
    if config.version != 2 || config.resource_id != id {
        return Err(Error::Binding(
            "linked resource configuration identity differs",
        ));
    }
    A::validate_target(&config.target, &row)?;
    Ok(config.target)
}

pub(super) fn verify(runtime: &mut Runtime<impl Host>, pipeline: &Target) -> Result<(), Error> {
    if pipeline.environment_id != "production" {
        return Err(Error::Binding(
            "media API supports only the production environment",
        ));
    }
    let api = target::<Api>(runtime, &pipeline.api_resource_id)?;
    let bucket = target::<Bucket>(runtime, &pipeline.bucket_resource_id)?;
    let result = runtime.bearer_json(&api.token_secret, "POST",
        "https://backboard.railway.com/graphql/v2".into(),
        json!({"query":"query DsfMediaApiDomain($serviceId:String!,$environmentId:String!){service(id:$serviceId){id projectId} serviceInstance(serviceId:$serviceId,environmentId:$environmentId){serviceId environmentId domains{customDomains{id domain projectId serviceId environmentId deletedAt}}}}",
            "variables":{"serviceId":api.service_id,"environmentId":api.environment_id}}))?;
    if result
        .get("errors")
        .is_some_and(|errors| !errors.as_array().is_some_and(Vec::is_empty))
    {
        return Err(Error::Response("Railway domain query"));
    }
    let service = result
        .pointer("/data/service")
        .ok_or(Error::Response("Railway service"))?;
    let instance = result
        .pointer("/data/serviceInstance")
        .ok_or(Error::Response("Railway service instance"))?;
    if required(service, "id")? != api.service_id
        || required(service, "projectId")? != api.project_id
        || required(instance, "serviceId")? != api.service_id
        || required(instance, "environmentId")? != api.environment_id
    {
        return Err(Error::Binding("linked API provider identity differs"));
    }
    let domains = instance
        .pointer("/domains/customDomains")
        .and_then(Value::as_array)
        .filter(|domains| domains.len() <= 100)
        .ok_or(Error::Response("Railway custom domains"))?;
    if !domains.iter().any(|domain| {
        domain.get("domain").and_then(Value::as_str) == Some("api.deep-sci-fi.world")
            && domain.get("projectId").and_then(Value::as_str) == Some(api.project_id.as_str())
            && domain.get("serviceId").and_then(Value::as_str) == Some(api.service_id.as_str())
            && domain.get("environmentId").and_then(Value::as_str)
                == Some(api.environment_id.as_str())
            && domain.get("deletedAt").is_some_and(Value::is_null)
    }) {
        return Err(Error::Binding(
            "linked API does not own the production domain",
        ));
    }
    let bucket_url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{}/r2/buckets/{}",
        encoded(&bucket.account_id),
        encoded(&bucket.bucket_name)
    );
    let metadata =
        runtime.bearer_json(&bucket.token_secret, "GET", bucket_url.clone(), Value::Null)?;
    if metadata.get("success") != Some(&Value::Bool(true))
        || metadata.pointer("/result/name").and_then(Value::as_str)
            != Some(bucket.bucket_name.as_str())
    {
        return Err(Error::Binding("linked media bucket identity differs"));
    }
    let response = runtime.bearer_json(
        &bucket.token_secret,
        "GET",
        format!("{bucket_url}/domains/custom"),
        Value::Null,
    )?;
    if response.get("success") != Some(&Value::Bool(true)) {
        return Err(Error::Response("R2 custom domains"));
    }
    let domains = response
        .pointer("/result/domains")
        .and_then(Value::as_array)
        .filter(|domains| domains.len() <= 100)
        .ok_or(Error::Response("R2 custom domains"))?;
    if !domains.iter().any(|domain| {
        domain.get("domain").and_then(Value::as_str) == Some("media.deep-sci-fi.world")
            && domain.get("enabled") == Some(&Value::Bool(true))
            && domain.pointer("/status/ownership").and_then(Value::as_str) == Some("active")
            && domain.pointer("/status/ssl").and_then(Value::as_str) == Some("active")
    }) {
        return Err(Error::Binding(
            "linked bucket does not serve the production media domain",
        ));
    }
    Ok(())
}

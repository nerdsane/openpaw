//! Platform IDENTITY spec bootstrap for the TemperPaw production host (ARN-255).
//!
//! # Why this exists
//!
//! The temper kernel ships two platform IDENTITY entity specs — `TrustedIssuer`
//! and `PrincipalGeneration` — that back kernel JWT verification (RFC-0002,
//! ARN-255). Upstream they are registered as part of `bootstrap_agent_specs`
//! (the built-in default agent-spec set).
//!
//! On the TemperPaw production host that default bootstrap is **skipped**: the
//! `paw-agent` OS app owns the agent lifecycle (`Agent`, `Session`, `Team`, …),
//! so registering the built-in defaults on top would be redundant and — because
//! it runs the full Z3 + Stateright + proptest cascade for ten specs — risks OOM
//! on the 512 MB Railway container (see `bootstrap_tenant_specs` in temper). That
//! skip is correct for the lifecycle specs.
//!
//! The catch: `paw-agent` does **not** own `TrustedIssuer` / `PrincipalGeneration`,
//! so the skip drops them too. In production the identity entity set is therefore
//! never registered (`GET .../tdata/TrustedIssuer` → `EntitySetNotFound`) and
//! `temper_platform::bootstrap_trusted_issuer_from_env` can't register an issuer —
//! ARN-255 human/agent JWT verification is inert.
//!
//! This module registers *only* those two identity specs, so the lifecycle skip
//! is preserved while the identity entities load regardless. It registers them
//! cascade-free (marking them verified the same way OS-app install does) so it
//! does not reintroduce the OOM that the default-bootstrap skip exists to avoid;
//! CI/offline gates own the deeper formal proof of these pinned specs.
//!
//! # Mirror note
//!
//! There is no narrower public temper API to register a subset of the agent
//! specs (`bootstrap_agent_specs` is all-or-nothing and runs the cascade), and
//! the spec sources are private to `temper-platform`. The three constants below
//! are therefore mirrored **verbatim** from temper rev
//! `a6e6289902b4d43c83a46882814f36cd56f7e1aa`:
//!   - `crates/temper-platform/src/specs/trusted_issuer.ioa.toml`
//!   - `crates/temper-platform/src/specs/principal_generation.ioa.toml`
//!   - the `TrustedIssuer` / `PrincipalGeneration` slice of
//!     `crates/temper-platform/src/specs/agent_model.csdl.xml`
//!
//! When the temper pin bumps, re-check these against the kernel specs. The clean
//! long-term fix is a narrow temper bootstrap API (e.g.
//! `bootstrap_identity_specs`) that this host can call without mirroring; until
//! that lands, this mirror keeps the pin change self-contained to temperpaw.

use temper_platform::PlatformState;
use temper_runtime::scheduler::sim_now;
use temper_runtime::tenant::TenantId;
use temper_server::registry::{EntityLevelSummary, EntityVerificationResult, VerificationStatus};
use temper_spec::csdl::parse_csdl;

/// Mirror of temper `specs/trusted_issuer.ioa.toml` @ rev a6e6289.
const TRUSTED_ISSUER_IOA: &str = r#"
[automaton]
name = "TrustedIssuer"
states = ["Active", "Suspended", "Revoked"]
initial = "Active"

[[state]]
name = "issuer"
type = "string"
initial = ""

[[state]]
name = "jwks_json"
type = "string"
initial = ""

[[state]]
name = "audience"
type = "string"
initial = ""

[[state]]
name = "algorithms"
type = "string"
initial = "ES256"

[[state]]
name = "description"
type = "string"
initial = ""

[[state]]
name = "created_by"
type = "string"
initial = ""

[[action]]
name = "RegisterIssuer"
kind = "input"
from = ["Active"]
params = ["issuer", "jwks_json", "audience", "algorithms", "description", "created_by"]
hint = "Register a trusted JWT issuer for this tenant. Entity id must be the issuer URL."

[[action]]
name = "RotateIssuerKeys"
kind = "input"
from = ["Active"]
params = ["jwks_json"]
hint = "Replace the issuer's inline JWKS with a rotated key set."

[[action]]
name = "SuspendIssuer"
kind = "input"
from = ["Active"]
to = "Suspended"
hint = "Temporarily stop trusting tokens from this issuer without discarding its registration."

[[action]]
name = "ResumeIssuer"
kind = "input"
from = ["Suspended"]
to = "Active"
hint = "Resume trusting tokens from a suspended issuer."

[[action]]
name = "RevokeIssuer"
kind = "input"
from = ["Active", "Suspended"]
to = "Revoked"
hint = "Permanently stop trusting this issuer."

[[invariant]]
name = "ActiveRequiresIssuer"
when = ["Active"]
assert = "issuer != ''"

[[invariant]]
name = "ActiveRequiresJwks"
when = ["Active"]
assert = "jwks_json != ''"

[[invariant]]
name = "ActiveRequiresAudience"
when = ["Active"]
assert = "audience != ''"
"#;

/// Mirror of temper `specs/principal_generation.ioa.toml` @ rev a6e6289.
const PRINCIPAL_GENERATION_IOA: &str = r#"
[automaton]
name = "PrincipalGeneration"
states = ["Active"]
initial = "Active"

[[state]]
name = "generation"
type = "counter"
initial = "0"

[[action]]
name = "BumpGeneration"
kind = "input"
from = ["Active"]
effect = [
  { type = "increment", var = "generation" },
  { type = "emit", event = "GenerationBumped" }
]
hint = "Sign out everywhere: invalidate every token issued for this principal before now by advancing its generation."
"#;

/// Mirror of the `TrustedIssuer` / `PrincipalGeneration` slice of temper
/// `specs/agent_model.csdl.xml` @ rev a6e6289 (namespace + container names kept
/// identical so OData routing matches a temper-cli deployment exactly).
const IDENTITY_CSDL: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<edmx:Edmx Version="4.0" xmlns:edmx="http://docs.oasis-open.org/odata/ns/edmx">
  <edmx:DataServices>
    <Schema Namespace="Temper" xmlns="http://docs.oasis-open.org/odata/ns/edm">

      <EntityType Name="TrustedIssuer">
        <Key><PropertyRef Name="id"/></Key>
        <Property Name="id" Type="Edm.String" Nullable="false"/>
        <Property Name="Status" Type="Edm.String"/>
        <Property Name="issuer" Type="Edm.String"/>
        <Property Name="jwks_json" Type="Edm.String"/>
        <Property Name="audience" Type="Edm.String"/>
        <Property Name="algorithms" Type="Edm.String"/>
        <Property Name="description" Type="Edm.String"/>
        <Property Name="created_by" Type="Edm.String"/>
      </EntityType>

      <EntityType Name="PrincipalGeneration">
        <Key><PropertyRef Name="id"/></Key>
        <Property Name="id" Type="Edm.String" Nullable="false"/>
        <Property Name="Status" Type="Edm.String"/>
        <Property Name="generation" Type="Edm.Int64"/>
      </EntityType>

      <Action Name="RegisterIssuer" IsBound="true">
        <Parameter Name="bindingParameter" Type="Temper.TrustedIssuer"/>
        <Parameter Name="issuer" Type="Edm.String" Nullable="false"/>
        <Parameter Name="jwks_json" Type="Edm.String" Nullable="false"/>
        <Parameter Name="audience" Type="Edm.String" Nullable="false"/>
        <Parameter Name="algorithms" Type="Edm.String" Nullable="true"/>
        <Parameter Name="description" Type="Edm.String" Nullable="true"/>
        <Parameter Name="created_by" Type="Edm.String" Nullable="true"/>
        <ReturnType Type="Temper.TrustedIssuer"/>
      </Action>

      <Action Name="RotateIssuerKeys" IsBound="true">
        <Parameter Name="bindingParameter" Type="Temper.TrustedIssuer"/>
        <Parameter Name="jwks_json" Type="Edm.String" Nullable="false"/>
        <ReturnType Type="Temper.TrustedIssuer"/>
      </Action>

      <Action Name="SuspendIssuer" IsBound="true">
        <Parameter Name="bindingParameter" Type="Temper.TrustedIssuer"/>
        <ReturnType Type="Temper.TrustedIssuer"/>
      </Action>

      <Action Name="ResumeIssuer" IsBound="true">
        <Parameter Name="bindingParameter" Type="Temper.TrustedIssuer"/>
        <ReturnType Type="Temper.TrustedIssuer"/>
      </Action>

      <Action Name="RevokeIssuer" IsBound="true">
        <Parameter Name="bindingParameter" Type="Temper.TrustedIssuer"/>
        <ReturnType Type="Temper.TrustedIssuer"/>
      </Action>

      <Action Name="BumpGeneration" IsBound="true">
        <Parameter Name="bindingParameter" Type="Temper.PrincipalGeneration"/>
        <ReturnType Type="Temper.PrincipalGeneration"/>
      </Action>

      <EntityContainer Name="Service">
        <EntitySet Name="TrustedIssuers" EntityType="Temper.TrustedIssuer"/>
        <EntitySet Name="PrincipalGenerations" EntityType="Temper.PrincipalGeneration"/>
      </EntityContainer>

    </Schema>
  </edmx:DataServices>
</edmx:Edmx>"#;

const IDENTITY_SPECS: &[(&str, &str)] = &[
    ("TrustedIssuer", TRUSTED_ISSUER_IOA),
    ("PrincipalGeneration", PRINCIPAL_GENERATION_IOA),
];

/// Register the platform IDENTITY specs (`TrustedIssuer`, `PrincipalGeneration`)
/// for `tenant`.
///
/// Merges the identity CSDL/IOA into the tenant's existing registration
/// (`merge = true`), so it is additive — it never wipes the entities or schema
/// that `paw-agent` (or any other OS app) owns. The specs are marked verified
/// without running the verification cascade, mirroring how OS-app install marks
/// its bundled specs; this avoids the memory cost the default-bootstrap skip
/// exists to prevent. Failures are logged and surfaced but do not panic — a
/// failed identity registration must not take down startup.
pub(crate) fn register_platform_identity_specs(state: &PlatformState, tenant: &str) {
    let csdl = match parse_csdl(IDENTITY_CSDL) {
        Ok(doc) => doc,
        Err(error) => {
            tracing::error!(
                %error,
                tenant,
                "Failed to parse platform identity CSDL; TrustedIssuer/PrincipalGeneration NOT registered — kernel JWT verification will be inert"
            );
            return;
        }
    };

    let tenant_id = TenantId::new(tenant);
    let mut registry = state.registry.write().unwrap(); // ci-ok: infallible startup lock

    if let Err(error) = registry.try_register_tenant_with_reactions_and_constraints(
        tenant_id.clone(),
        csdl,
        IDENTITY_CSDL.to_string(),
        IDENTITY_SPECS,
        Vec::new(),
        None,
        true, // merge: additive; preserve paw-agent's app entities and schema
    ) {
        tracing::error!(
            %error,
            tenant,
            "Failed to register platform identity specs; kernel JWT verification will be inert"
        );
        return;
    }

    // Trust-register: mark verified without the Z3 + Stateright + proptest
    // cascade, exactly as OS-app install does. Running the full cascade here is
    // precisely what the skipped default agent-specs bootstrap avoids to stay
    // within the 512 MB Railway container.
    let verified_at = sim_now().to_rfc3339();
    for (entity_type, _) in IDENTITY_SPECS {
        registry.set_verification_status(
            &tenant_id,
            entity_type,
            VerificationStatus::Completed(EntityVerificationResult {
                all_passed: true,
                levels: vec![EntityLevelSummary {
                    level: "Bootstrap".to_string(),
                    passed: true,
                    summary: "Pre-verified platform identity spec at bootstrap".to_string(),
                    details: None,
                }],
                verified_at: verified_at.clone(),
            }),
        );
    }

    tracing::info!(
        tenant,
        "Registered platform identity specs (TrustedIssuer, PrincipalGeneration) despite skipped default agent-specs bootstrap"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_ioa_specs_parse() {
        for (name, source) in IDENTITY_SPECS {
            temper_spec::automaton::parse_automaton(source)
                .unwrap_or_else(|e| panic!("identity spec {name} failed to parse: {e}"));
        }
    }

    #[test]
    fn identity_csdl_parses_with_both_entity_sets() {
        let doc = parse_csdl(IDENTITY_CSDL).expect("identity CSDL should parse");
        let schema = doc
            .schemas
            .iter()
            .find(|s| s.namespace == "Temper")
            .expect("Temper namespace present");
        assert!(schema.entity_types.iter().any(|e| e.name == "TrustedIssuer"));
        assert!(
            schema
                .entity_types
                .iter()
                .any(|e| e.name == "PrincipalGeneration")
        );
        let container = &schema.entity_containers[0];
        assert!(
            container
                .entity_sets
                .iter()
                .any(|s| s.name == "TrustedIssuers")
        );
        assert!(
            container
                .entity_sets
                .iter()
                .any(|s| s.name == "PrincipalGenerations")
        );
    }

    #[test]
    fn register_platform_identity_specs_registers_both_entities() {
        let state = PlatformState::new(None);
        register_platform_identity_specs(&state, "app-tenant");

        let registry = state.registry.read().unwrap();
        let tenant = TenantId::new("app-tenant");
        assert!(
            registry.get_table(&tenant, "TrustedIssuer").is_some(),
            "TrustedIssuer entity should be registered"
        );
        assert!(
            registry.get_table(&tenant, "PrincipalGeneration").is_some(),
            "PrincipalGeneration entity should be registered"
        );
        assert_eq!(
            registry
                .resolve_entity_type(&tenant, "TrustedIssuers")
                .as_deref(),
            Some("TrustedIssuer"),
            "TrustedIssuers set should map to the TrustedIssuer type"
        );
    }
}

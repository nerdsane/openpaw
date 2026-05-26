fn parse_worker_command(args: impl IntoIterator<Item = String>) -> WorkerCommand {
    for arg in args {
        match arg.as_str() {
            "doctor" | "--doctor" => return WorkerCommand::Doctor,
            "launchd-plist" | "plist" | "--launchd-plist" => {
                return WorkerCommand::LaunchdPlist;
            }
            "run" | "--run" => return WorkerCommand::Run,
            "directed-evolution-demo" | "--directed-evolution-demo" => {
                return WorkerCommand::DirectedEvolutionDemo;
            }
            _ => {}
        }
    }
    WorkerCommand::Run
}

fn worker_run_is_repo_sweep(worker_run: &WorkerRunState) -> bool {
    extract_repo_sweep_snapshot_id(&worker_run.task).is_some()
}

fn first_string(value: &Value, fields: &Value, top_keys: &[&str], field_keys: &[&str]) -> String {
    top_keys
        .iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .or_else(|| {
            field_keys
                .iter()
                .find_map(|key| fields.get(*key).and_then(Value::as_str))
        })
        .unwrap_or("")
        .to_string()
}

fn first_bool(value: &Value, fields: &Value, top_keys: &[&str], field_keys: &[&str]) -> bool {
    top_keys
        .iter()
        .find_map(|key| value.get(*key).and_then(value_as_bool))
        .or_else(|| {
            field_keys
                .iter()
                .find_map(|key| fields.get(*key).and_then(value_as_bool))
        })
        .unwrap_or(false)
}

fn value_as_bool(value: &Value) -> Option<bool> {
    value.as_bool().or_else(|| {
        value
            .as_str()
            .map(|value| value.eq_ignore_ascii_case("true"))
    })
}

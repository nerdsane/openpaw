impl DoctorCheck {
    fn pass(name: &str, detail: String) -> Self {
        Self {
            name: name.to_string(),
            status: DoctorStatus::Pass,
            detail,
        }
    }

    fn warn(name: &str, detail: String) -> Self {
        Self {
            name: name.to_string(),
            status: DoctorStatus::Warn,
            detail,
        }
    }

    fn fail(name: &str, detail: String) -> Self {
        Self {
            name: name.to_string(),
            status: DoctorStatus::Fail,
            detail,
        }
    }
}

fn doctor_has_failures(checks: &[DoctorCheck]) -> bool {
    checks
        .iter()
        .any(|check| check.status == DoctorStatus::Fail)
}

fn print_doctor_report(config: &Config, checks: &[DoctorCheck]) {
    println!("paw-codex-worker doctor");
    println!("  worker_id: {}", config.worker_id);
    println!("  temper_url: {}", config.temper_url);
    println!("  tenant: {}", config.tenant);
    for check in checks {
        println!(
            "  [{}] {}: {}",
            doctor_status_label(check.status),
            check.name,
            check.detail
        );
    }
}


#[cfg(test)]
mod directed_evolution_prompt_tests {
    use super::*;
    use std::ffi::OsString;

    struct EnvOverride {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvOverride {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var_os(key);
            unsafe {
                env::set_var(key, value);
            }
            Self { key, previous }
        }
    }

    impl Drop for EnvOverride {
        fn drop(&mut self) {
            unsafe {
                match &self.previous {
                    Some(value) => env::set_var(self.key, value),
                    None => env::remove_var(self.key),
                }
            }
        }
    }

    #[test]
    fn evaluation_prompt_rewrites_loopback_api_base() {
        let _url = EnvOverride::set(
            "DIRECTED_EVOLUTION_PUBLIC_API_URL",
            "https://genesis.example.test/",
        );
        let body = directed_evolution_worker_prompt_body(
            "simulated_user",
            "Evaluate\nTemperApiBase: http://127.0.0.1:8080\nRuntimeRef: temper://tenant/t/app/a",
        );

        assert!(body.contains("TemperApiBase: https://genesis.example.test"));
        assert!(body.contains("do not assume localhost is the target runtime"));
        assert!(body.contains("Do not start a foreground long-lived server"));
        assert!(!body.contains("TemperApiBase: http://127.0.0.1:8080"));
    }

    #[test]
    fn variant_generator_prompt_is_not_rewritten() {
        let _url = EnvOverride::set(
            "DIRECTED_EVOLUTION_PUBLIC_API_URL",
            "https://genesis.example.test",
        );
        let body = directed_evolution_worker_prompt_body(
            "variant_generator",
            "TemperApiBase: http://127.0.0.1:8080",
        );

        assert_eq!(body, "TemperApiBase: http://127.0.0.1:8080");
    }
}

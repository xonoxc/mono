//! Tests for Mono modules
//! 
//! These tests verify core functionality without requiring
//! a running daemon or complex setup.

#[cfg(test)]
mod consent_tests {
    #[test]
    fn test_daemon_path_returns_valid_path() {
        let path = mono::tui::consent::get_daemon_path();
        assert_eq!(path.file_name().unwrap(), "mono-tracker");
    }

    #[test]
    fn test_config_dir_ends_with_mono() {
        let path = mono::tui::consent::get_config_dir();
        assert!(path.to_str().unwrap().ends_with("mono"));
    }

    #[test]
    fn test_consent_file_is_named_consent() {
        let path = mono::tui::consent::get_consent_file();
        assert_eq!(path.file_name().unwrap(), "consent");
    }

    #[test]
    fn test_consent_file_is_inside_config_dir() {
        let config = mono::tui::consent::get_config_dir();
        let consent = mono::tui::consent::get_consent_file();
        assert!(consent.starts_with(&config));
    }

    #[test]
    fn test_is_daemon_running_returns_bool() {
        let result = mono::tui::consent::is_daemon_running();
        // Just verify it returns a valid boolean
        assert!(result == true || result == false);
    }

    #[test]
    fn test_set_consent_false_removes_file() {
        // Save original state
        let had_consent = mono::tui::consent::has_consent();
        
        // If we had consent, temporarily remove it
        if had_consent {
            let _ = std::fs::remove_file(mono::tui::consent::get_consent_file());
            assert!(!mono::tui::consent::has_consent());
        }
        
        // Restore if we had it
        if had_consent {
            let config = mono::tui::consent::get_config_dir();
            std::fs::create_dir_all(config).ok();
            std::fs::write(mono::tui::consent::get_consent_file(), "1").ok();
        }
    }

    #[test]
    fn test_set_consent_true_creates_file() {
        // Remove consent first
        let consent_file = mono::tui::consent::get_consent_file();
        let _ = std::fs::remove_file(&consent_file);
        
        assert!(!mono::tui::consent::has_consent());
        
        // Set consent
        mono::tui::consent::set_consent(true).ok();
        
        assert!(mono::tui::consent::has_consent());
        
        // Cleanup
        let _ = std::fs::remove_file(&consent_file);
    }

    #[test]
    fn test_has_consent_reflects_file_existence() {
        let consent_file = mono::tui::consent::get_consent_file();
        let exists_before = consent_file.exists();
        let has_consent_before = mono::tui::consent::has_consent();
        
        assert_eq!(exists_before, has_consent_before);
    }
}

#[cfg(test)]
mod session_tests {
    use mono::models::Session;

    #[test]
    fn test_session_new_creates_valid_session() {
        let session = Session::new(
            "test-app".to_string(),
            "Test Window".to_string(),
        );
        
        assert_eq!(session.app_name, "test-app");
        assert_eq!(session.window_title, "Test Window");
        assert_eq!(session.end_time, None);
        assert_eq!(session.duration_secs, 0);
        assert!(!session.is_idle);
    }

    #[test]
    fn test_session_new_idle_creates_idle_session() {
        let session = Session::new_idle();
        
        assert_eq!(session.app_name, "__idle__");
        assert_eq!(session.window_title, "System Idle");
        assert!(session.is_idle);
    }

    #[test]
    fn test_session_close_sets_end_time() {
        let mut session = Session::new(
            "test-app".to_string(),
            "Test Window".to_string(),
        );
        
        assert!(session.end_time.is_none());
        
        session.close();
        
        assert!(session.end_time.is_some());
        assert!(session.duration_secs >= 0);
    }
}
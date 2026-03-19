//! Persona definitions for multi-user flow crawling.
//!
//! Personas represent different user types (anonymous, free, pro) and carry
//! cookies and credentials that shape how the app behaves during crawling.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// A user persona for flow crawling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    /// Human-readable name (e.g., "Pro User").
    pub name: String,
    /// Short description of this persona.
    #[serde(default)]
    pub description: String,
    /// Hex color for visualization (e.g., "#f59e0b").
    #[serde(default = "default_color")]
    pub color: String,
    /// Cookies to inject before crawling.
    #[serde(default)]
    pub cookies: Vec<PersonaCookie>,
    /// Login credentials (used if cookies aren't sufficient).
    #[serde(default)]
    pub credentials: Option<Credentials>,
    /// Tags for filtering and grouping.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A cookie to inject for a persona.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaCookie {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub secure: Option<bool>,
    #[serde(default)]
    pub http_only: Option<bool>,
}

/// Login credentials for a persona.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}

fn default_color() -> String {
    "#6b7280".to_string()
}

/// Default color palette for auto-generated personas.
const PERSONA_COLORS: &[&str] = &[
    "#6b7280", // gray (anonymous)
    "#22c55e", // green (authenticated)
    "#3b82f6", // blue
    "#f59e0b", // amber
    "#ef4444", // red
    "#8b5cf6", // purple
    "#ec4899", // pink
    "#14b8a6", // teal
];

/// Load personas from a directory of `.persona.yaml` files.
pub fn load_personas(dir: &Path) -> Vec<Persona> {
    if !dir.exists() {
        return Vec::new();
    }

    let pattern = dir.join("*.persona.yaml");
    let pattern_str = pattern.to_string_lossy();

    let mut personas = Vec::new();
    if let Ok(paths) = glob::glob(&pattern_str) {
        for entry in paths.flatten() {
            match std::fs::read_to_string(&entry) {
                Ok(content) => match serde_yaml::from_str::<Persona>(&content) {
                    Ok(persona) => {
                        tracing::info!("Loaded persona: {} from {}", persona.name, entry.display());
                        personas.push(persona);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse {}: {e}", entry.display());
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read {}: {e}", entry.display());
                }
            }
        }
    }

    personas
}

/// Generate default personas when none are configured.
pub fn default_personas() -> Vec<Persona> {
    vec![
        Persona {
            name: "Anonymous".to_string(),
            description: "First-time visitor, no account".to_string(),
            color: PERSONA_COLORS[0].to_string(),
            cookies: Vec::new(),
            credentials: None,
            tags: vec!["unauthenticated".to_string()],
        },
        Persona {
            name: "Authenticated".to_string(),
            description: "Logged-in user (cookies imported from real browser)".to_string(),
            color: PERSONA_COLORS[1].to_string(),
            cookies: Vec::new(), // Filled at crawl time via auto-auth
            credentials: None,
            tags: vec!["authenticated".to_string()],
        },
    ]
}

/// Write personas to disk so they're editable.
pub fn write_default_personas(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;

    for (i, persona) in default_personas().iter().enumerate() {
        let filename = format!(
            "{}.persona.yaml",
            persona.name.to_lowercase().replace(' ', "-")
        );
        let path = dir.join(&filename);
        if path.exists() {
            continue;
        }

        let yaml = serde_yaml::to_string(persona).unwrap_or_default();
        std::fs::write(&path, yaml)?;
        tracing::info!("Wrote default persona: {}", path.display());

        // Ensure color is assigned from palette
        let _ = i; // palette index if needed
    }

    Ok(())
}

/// Convert persona cookies to rayo-core SetCookie format.
pub fn to_set_cookies(
    cookies: &[PersonaCookie],
    default_domain: Option<&str>,
) -> Vec<rayo_core::cookie::SetCookie> {
    cookies
        .iter()
        .map(|c| rayo_core::cookie::SetCookie {
            name: c.name.clone(),
            value: c.value.clone(),
            domain: c
                .domain
                .clone()
                .or_else(|| default_domain.map(String::from)),
            path: c.path.clone(),
            url: None,
            secure: c.secure,
            http_only: c.http_only,
            same_site: None,
            expires: None,
        })
        .collect()
}

/// Assign persona color from palette if it uses the default.
pub fn assign_color(persona: &mut Persona, index: usize) {
    if persona.color == default_color() && index < PERSONA_COLORS.len() {
        persona.color = PERSONA_COLORS[index].to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_personas() {
        let personas = default_personas();
        assert_eq!(personas.len(), 2);
        assert_eq!(personas[0].name, "Anonymous");
        assert_eq!(personas[1].name, "Authenticated");
        assert!(personas[0].cookies.is_empty());
    }

    #[test]
    fn test_persona_yaml_roundtrip() {
        let persona = Persona {
            name: "Pro User".to_string(),
            description: "Paid subscriber".to_string(),
            color: "#f59e0b".to_string(),
            cookies: vec![PersonaCookie {
                name: "session".to_string(),
                value: "abc123".to_string(),
                domain: Some(".example.com".to_string()),
                path: None,
                secure: Some(true),
                http_only: None,
            }],
            credentials: Some(Credentials {
                email: "pro@example.com".to_string(),
                password: "pass123".to_string(),
            }),
            tags: vec!["pro".to_string(), "authenticated".to_string()],
        };

        let yaml = serde_yaml::to_string(&persona).unwrap();
        let parsed: Persona = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.name, "Pro User");
        assert_eq!(parsed.cookies.len(), 1);
        assert_eq!(parsed.cookies[0].name, "session");
        assert!(parsed.credentials.is_some());
    }

    #[test]
    fn test_to_set_cookies() {
        let cookies = vec![PersonaCookie {
            name: "tok".to_string(),
            value: "val".to_string(),
            domain: None,
            path: None,
            secure: None,
            http_only: None,
        }];

        let set_cookies = to_set_cookies(&cookies, Some("example.com"));
        assert_eq!(set_cookies.len(), 1);
        assert_eq!(set_cookies[0].name, "tok");
        assert_eq!(set_cookies[0].domain, Some("example.com".to_string()));
    }

    #[test]
    fn test_load_personas_empty_dir() {
        let dir = std::env::temp_dir().join("rayo_test_personas_empty");
        let _ = std::fs::create_dir_all(&dir);
        let personas = load_personas(&dir);
        assert!(personas.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_personas_nonexistent_dir() {
        let personas = load_personas(Path::new("/nonexistent/path"));
        assert!(personas.is_empty());
    }
}

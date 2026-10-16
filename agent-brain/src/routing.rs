use crate::projects::Project;
use regex::Regex;
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

pub const SLUG_MAX_LEN: usize = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteKind {
    None,
    Ambiguous,
}

#[derive(Debug, Clone)]
pub struct RoutingError {
    pub kind: RouteKind,
    pub candidates: Vec<Project>,
}

impl std::fmt::Display for RoutingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Routing error ({:?}): {} candidates",
            self.kind,
            self.candidates.len()
        )
    }
}

impl std::error::Error for RoutingError {}

pub fn normalize(text: &str) -> String {
    static RE_SPACE: OnceLock<Regex> = OnceLock::new();
    static RE_MN: OnceLock<Regex> = OnceLock::new();
    let re_space = RE_SPACE.get_or_init(|| Regex::new(r"[\s_\-]+").unwrap());
    let re_mn = RE_MN.get_or_init(|| Regex::new(r"\p{Mn}").unwrap());

    let s_nfd: String = text.replace(['đ', 'Đ'], "d").nfd().collect();
    let s_clean = re_mn.replace_all(&s_nfd, "");
    re_space
        .replace_all(&s_clean.to_lowercase(), " ")
        .trim()
        .to_string()
}

pub fn slugify(text: &str) -> String {
    static RE_INVALID: OnceLock<Regex> = OnceLock::new();
    static RE_HYPHENS: OnceLock<Regex> = OnceLock::new();
    let re_invalid = RE_INVALID.get_or_init(|| Regex::new(r"[^a-z0-9\-]+").unwrap());
    let re_hyphens = RE_HYPHENS.get_or_init(|| Regex::new(r"-+").unwrap());

    let norm = normalize(text);
    if norm.is_empty() {
        return String::new();
    }
    let slug = norm.replace(' ', "-");
    let slug = re_invalid.replace_all(&slug, "");
    let slug = re_hyphens.replace_all(&slug, "-");
    let slug = slug.trim_matches('-');
    if slug.len() <= SLUG_MAX_LEN {
        return slug.to_string();
    }
    let mut truncated = &slug[..SLUG_MAX_LEN];
    if let Some(last_hyphen) = truncated.rfind('-') {
        if last_hyphen > 0 {
            truncated = &truncated[..last_hyphen];
        }
    }
    truncated.trim_matches('-').to_string()
}

pub fn resolve(query: &str, projects: &[Project]) -> Result<Project, RoutingError> {
    let stripped_query = query.trim();
    if stripped_query.is_empty() {
        return Err(RoutingError {
            kind: RouteKind::None,
            candidates: projects.to_vec(),
        });
    }

    for p in projects {
        if p.id.trim() == stripped_query {
            return Ok(p.clone());
        }
    }

    let norm_q = normalize(stripped_query);
    let mut matches = Vec::new();
    for p in projects {
        let mut matched = false;
        if normalize(&p.id) == norm_q {
            matched = true;
        } else if normalize(&p.name_vi) == norm_q {
            matched = true;
        } else {
            for alias in &p.aliases {
                if normalize(alias) == norm_q {
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            matches.push(p.clone());
        }
    }

    if matches.len() == 1 {
        Ok(matches.remove(0))
    } else if matches.is_empty() {
        Err(RoutingError {
            kind: RouteKind::None,
            candidates: projects.to_vec(),
        })
    } else {
        Err(RoutingError {
            kind: RouteKind::Ambiguous,
            candidates: matches,
        })
    }
}

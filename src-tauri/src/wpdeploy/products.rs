//! Resolve which products a submodule repo ships, driven entirely by the
//! user-configured product map (Settings) plus the monorepo's product-slugs JSON
//! (the single source of truth for slugs). Nothing product-specific is hardcoded
//! here: the repo->group map, the theme slug and the slugs JSON path all come
//! from `WpDeployConfig`, so a fresh install ships empty and each user points it
//! at their own monorepo.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use super::commands::group_is_buildable;
use super::{Product, WpDeployConfig};

/// One product group from the product-slugs JSON. `lite` is a single slug
/// (absent for some groups); `pro` is a list (may be empty).
#[derive(Debug, Clone, Deserialize)]
struct Group {
    #[serde(default)]
    lite: Option<String>,
    #[serde(default)]
    pro: Vec<String>,
}

type Groups = BTreeMap<String, Group>;

enum Kind {
    Lite,
    Pro,
    Theme,
}

fn parse_kind(kind: &str) -> Option<Kind> {
    match kind {
        "lite" => Some(Kind::Lite),
        "pro" => Some(Kind::Pro),
        "theme" => Some(Kind::Theme),
        _ => None,
    }
}

fn slugs_path(cfg: &WpDeployConfig, monorepo: &str) -> Result<std::path::PathBuf, String> {
    let rel = cfg.slugs_rel_path.trim();
    if rel.is_empty() {
        return Err("product-slugs path not set — configure it in Settings".into());
    }
    Ok(Path::new(monorepo).join(rel))
}

fn load_groups(cfg: &WpDeployConfig, monorepo: &str) -> Result<Groups, String> {
    let path = slugs_path(cfg, monorepo)?;
    let data = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&data).map_err(|e| format!("bad product-slugs JSON: {e}"))
}

/// Products deployable from a given submodule repo folder. Empty when the repo
/// isn't in the configured map (nothing deployable).
pub fn products_for_repo(
    cfg: &WpDeployConfig,
    monorepo: &str,
    repo: &str,
) -> Result<Vec<Product>, String> {
    let Some(mapping) = cfg.repo_map.iter().find(|m| m.repo == repo) else {
        return Ok(Vec::new());
    };
    let Some(kind) = parse_kind(&mapping.kind) else {
        return Err(format!("bad mapping kind for {repo}: {}", mapping.kind));
    };
    if let Kind::Theme = kind {
        if cfg.theme_slug.trim().is_empty() {
            return Ok(Vec::new());
        }
        return Ok(vec![Product {
            buildable: group_is_buildable(&mapping.group),
            group: mapping.group.clone(),
            slug: cfg.theme_slug.clone(),
            is_lite: false,
        }]);
    }
    let groups = load_groups(cfg, monorepo)?;
    let Some(g) = groups.get(&mapping.group) else {
        return Ok(Vec::new());
    };
    let products = match kind {
        Kind::Lite => g
            .lite
            .iter()
            .map(|slug| Product {
                buildable: group_is_buildable(&mapping.group),
                group: mapping.group.clone(),
                slug: slug.clone(),
                is_lite: true,
            })
            .collect(),
        Kind::Pro => g
            .pro
            .iter()
            .map(|slug| Product {
                buildable: group_is_buildable(&mapping.group),
                group: mapping.group.clone(),
                slug: slug.clone(),
                is_lite: false,
            })
            .collect(),
        Kind::Theme => unreachable!(),
    };
    Ok(products)
}

/// Resolve a slug's group + lite flag (for build decisions during deploy).
pub fn resolve_slug(
    cfg: &WpDeployConfig,
    monorepo: &str,
    slug: &str,
) -> Result<(String, bool), String> {
    if !cfg.theme_slug.trim().is_empty() && slug == cfg.theme_slug {
        // Theme's group is whatever the map assigns to a `theme`-kind entry, else
        // just "theme".
        let group = cfg
            .repo_map
            .iter()
            .find(|m| m.kind == "theme")
            .map(|m| m.group.clone())
            .unwrap_or_else(|| "theme".to_string());
        return Ok((group, false));
    }
    let groups = load_groups(cfg, monorepo)?;
    for (name, g) in &groups {
        if g.lite.as_deref() == Some(slug) {
            return Ok((name.clone(), true));
        }
        if g.pro.iter().any(|s| s == slug) {
            return Ok((name.clone(), false));
        }
    }
    Err(format!("unknown product slug: {slug}"))
}

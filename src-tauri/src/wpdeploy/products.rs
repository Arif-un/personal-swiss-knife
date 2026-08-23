//! Read `envira-dev/dev/utils/src/product-slugs.json` (the single source of
//! truth for product slugs) and map a submodule repo folder to the products it
//! ships. New addons added to that JSON appear automatically.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use super::Product;

/// One product group from product-slugs.json. `lite` is a single slug (absent
/// for cdn); `pro` is a list (may be empty).
#[derive(Debug, Clone, Deserialize)]
struct Group {
    #[serde(default)]
    lite: Option<String>,
    #[serde(default)]
    pro: Vec<String>,
}

type Groups = BTreeMap<String, Group>;

/// The theme product is not in product-slugs.json (it lives in the theme repo).
pub const THEME_SLUG: &str = "imagely-theme";

fn slugs_path(envira_dev: &str) -> std::path::PathBuf {
    Path::new(envira_dev).join("dev/utils/src/product-slugs.json")
}

fn load_groups(envira_dev: &str) -> Result<Groups, String> {
    let path = slugs_path(envira_dev);
    let data = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&data).map_err(|e| format!("bad product-slugs.json: {e}"))
}

/// Repo folder name -> (group, kind). Folder names are fixed submodule paths;
/// the actual slugs come from the JSON so addon changes need no code change.
enum Kind {
    Lite,
    Pro,
    Theme,
}

fn repo_kind(repo: &str) -> Option<(&'static str, Kind)> {
    match repo {
        "envira-gallery-lite" => Some(("envira", Kind::Lite)),
        "envira-gallery-plugin" => Some(("envira", Kind::Pro)),
        "soliloquy-lite" => Some(("soliloquy", Kind::Lite)),
        "soliloquy-plugin" => Some(("soliloquy", Kind::Pro)),
        "nextgen-gallery" => Some(("nextgen", Kind::Lite)),
        "nextgen-gallery-pro" => Some(("nextgen", Kind::Pro)),
        "envira-image-cdn" => Some(("cdn", Kind::Pro)),
        "photocrati-10-theme" => Some(("theme", Kind::Theme)),
        _ => None,
    }
}

/// Products deployable from a given submodule repo folder. Empty when the repo
/// ships nothing deployable (e.g. `envira-dev`, `envira-dev-builds`).
pub fn products_for_repo(envira_dev: &str, repo: &str) -> Result<Vec<Product>, String> {
    let Some((group, kind)) = repo_kind(repo) else {
        return Ok(Vec::new());
    };
    if let Kind::Theme = kind {
        return Ok(vec![Product {
            group: "theme".into(),
            slug: THEME_SLUG.into(),
            is_lite: false,
        }]);
    }
    let groups = load_groups(envira_dev)?;
    let Some(g) = groups.get(group) else {
        return Ok(Vec::new());
    };
    let products = match kind {
        Kind::Lite => g
            .lite
            .iter()
            .map(|slug| Product {
                group: group.into(),
                slug: slug.clone(),
                is_lite: true,
            })
            .collect(),
        Kind::Pro => g
            .pro
            .iter()
            .map(|slug| Product {
                group: group.into(),
                slug: slug.clone(),
                is_lite: false,
            })
            .collect(),
        Kind::Theme => unreachable!(),
    };
    Ok(products)
}

/// Resolve a slug's group + lite flag (for build decisions during deploy).
pub fn resolve_slug(envira_dev: &str, slug: &str) -> Result<(String, bool), String> {
    if slug == THEME_SLUG {
        return Ok(("theme".into(), false));
    }
    let groups = load_groups(envira_dev)?;
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

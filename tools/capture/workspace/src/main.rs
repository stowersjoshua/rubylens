//! Reads a list of package specs and reports the newest compatible versions.
mod shapes;

use anyhow::{bail, Context, Result};
use itertools::Itertools;
use regex::Regex;
use semver::{Version, VersionReq};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
struct Spec {
    name: String,
    req: VersionReq,
}

fn parse_specs(text: &str) -> Result<Vec<Spec>> {
    let line_re = Regex::new(r"^\s*([a-z0-9_-]+)\s*=\s*(.+?)\s*$")?;
    let mut specs = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(caps) = line_re.captures(line) else {
            bail!("line {}: cannot parse {:?}", i + 1, line);
        };
        let req = VersionReq::parse(&caps[2]).with_context(|| format!("bad requirement on line {}", i + 1))?;
        specs.push(Spec { name: caps[1].to_string(), req });
    }
    Ok(specs)
}

fn newest_matching<'a>(req: &VersionReq, available: &'a [Version]) -> Option<&'a Version> {
    available.iter().filter(|v| req.matches(v)).max()
}

fn report(specs: &[Spec], index: &BTreeMap<String, Vec<Version>>) -> Value {
    let rows = specs
        .iter()
        .sorted_by_key(|s| s.name.clone())
        .map(|s| {
            let versions = index.get(&s.name).map(Vec::as_slice).unwrap_or(&[]);
            match newest_matching(&s.req, versions) {
                Some(v) => json!({ "name": s.name, "req": s.req.to_string(), "picked": v.to_string() }),
                None => json!({ "name": s.name, "req": s.req.to_string(), "picked": null }),
            }
        })
        .collect::<Vec<_>>();
    let unresolved = rows.iter().filter(|r| r["picked"].is_null()).count();
    json!({ "packages": rows, "unresolved": unresolved })
}

fn main() -> Result<()> {
    let mut index: BTreeMap<String, Vec<Version>> = BTreeMap::new();
    for (name, v) in [("serde", "1.0.200"), ("serde", "1.0.229"), ("regex", "1.11.0"), ("tokio", "1.53.2")] {
        index.entry(name.to_string()).or_default().push(Version::parse(v)?);
    }
    let specs = parse_specs("serde = ^1.0\nregex = >=1.10\nleft-pad = 2")?;
    let out = report(&specs, &index);
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

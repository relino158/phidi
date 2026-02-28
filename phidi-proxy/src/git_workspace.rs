use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::Result;
use git2::{DiffOptions, Oid, Repository};
use phidi_rpc::{
    agent::DeltaScope,
    source_control::{DiffInfo, FileDiff},
};

pub(crate) fn collect_working_tree_diffs(
    workspace_path: &Path,
    scope: DeltaScope,
) -> Result<Vec<FileDiff>> {
    let repo = Repository::discover(workspace_path)?;
    let mut deltas = Vec::new();

    if matches!(scope, DeltaScope::Unstaged | DeltaScope::All) {
        let mut diff_options = DiffOptions::new();
        let diff = repo.diff_index_to_workdir(
            None,
            Some(
                diff_options
                    .include_untracked(true)
                    .recurse_untracked_dirs(true),
            ),
        )?;
        for delta in diff.deltas() {
            if let Some(delta) = git_delta_format(workspace_path, &delta) {
                deltas.push(delta);
            }
        }
    }

    if matches!(scope, DeltaScope::Staged | DeltaScope::All) {
        let head_tree = repo
            .revparse_single("HEAD^{tree}")
            .ok()
            .and_then(|object| repo.find_tree(object.id()).ok());
        let diff = repo.diff_tree_to_index(head_tree.as_ref(), None, None)?;
        for delta in diff.deltas() {
            if let Some(delta) = git_delta_format(workspace_path, &delta) {
                deltas.push(delta);
            }
        }
    }

    Ok(merge_git_deltas(deltas))
}

pub(crate) fn diff_info(workspace_path: &Path) -> Option<DiffInfo> {
    let repo = Repository::discover(workspace_path).ok()?;
    let name = match repo.head() {
        Ok(head) => head.shorthand()?.to_string(),
        _ => "(No branch)".to_owned(),
    };

    let mut branches = Vec::new();
    for branch in repo.branches(None).ok()? {
        branches.push(branch.ok()?.0.name().ok()??.to_string());
    }

    let mut tags = Vec::new();
    if let Ok(git_tags) = repo.tag_names(None) {
        for tag in git_tags.into_iter().flatten() {
            tags.push(tag.to_owned());
        }
    }

    let diffs = collect_working_tree_diffs(workspace_path, DeltaScope::All).ok()?;

    Some(DiffInfo {
        head: name,
        branches,
        tags,
        diffs,
    })
}

type GitDelta = (git2::Delta, Oid, PathBuf);

fn merge_git_deltas(deltas: Vec<GitDelta>) -> Vec<FileDiff> {
    let mut renames = Vec::new();
    let mut renamed_deltas = HashSet::new();

    for (added_index, delta) in deltas.iter().enumerate() {
        if delta.0 == git2::Delta::Added {
            for (deleted_index, candidate) in deltas.iter().enumerate() {
                if candidate.0 == git2::Delta::Deleted && candidate.1 == delta.1 {
                    renames.push((added_index, deleted_index));
                    renamed_deltas.insert(added_index);
                    renamed_deltas.insert(deleted_index);
                    break;
                }
            }
        }
    }

    let mut merged = Vec::new();
    let mut by_path = BTreeMap::new();
    for (added_index, deleted_index) in renames {
        merged.push(FileDiff::Renamed(
            deltas[added_index].2.clone(),
            deltas[deleted_index].2.clone(),
        ));
    }
    for (index, delta) in deltas.iter().enumerate() {
        if renamed_deltas.contains(&index) {
            continue;
        }
        let diff = match delta.0 {
            git2::Delta::Added => FileDiff::Added(delta.2.clone()),
            git2::Delta::Deleted => FileDiff::Deleted(delta.2.clone()),
            git2::Delta::Modified => FileDiff::Modified(delta.2.clone()),
            _ => continue,
        };
        let key = diff_sort_key(&diff);
        if let Some(existing) = by_path.get(&key) {
            if diff_priority(&diff) <= diff_priority(existing) {
                continue;
            }
        }
        by_path.insert(key, diff);
    }
    merged.extend(by_path.into_values());
    merged.sort_by_key(diff_sort_key);
    merged
}

fn diff_sort_key(diff: &FileDiff) -> String {
    match diff {
        FileDiff::Modified(path)
        | FileDiff::Added(path)
        | FileDiff::Deleted(path)
        | FileDiff::Renamed(path, _) => path.to_string_lossy().into_owned(),
    }
}

fn diff_priority(diff: &FileDiff) -> u8 {
    match diff {
        FileDiff::Modified(_) => 0,
        FileDiff::Added(_) => 1,
        FileDiff::Deleted(_) => 1,
        FileDiff::Renamed(_, _) => 2,
    }
}

fn git_delta_format(
    workspace_path: &Path,
    delta: &git2::DiffDelta,
) -> Option<GitDelta> {
    match delta.status() {
        git2::Delta::Added | git2::Delta::Untracked => Some((
            git2::Delta::Added,
            delta.new_file().id(),
            delta
                .new_file()
                .path()
                .map(|path| workspace_path.join(path))?,
        )),
        git2::Delta::Deleted => Some((
            git2::Delta::Deleted,
            delta.old_file().id(),
            delta
                .old_file()
                .path()
                .map(|path| workspace_path.join(path))?,
        )),
        git2::Delta::Modified => Some((
            git2::Delta::Modified,
            delta.new_file().id(),
            delta
                .new_file()
                .path()
                .map(|path| workspace_path.join(path))?,
        )),
        _ => None,
    }
}

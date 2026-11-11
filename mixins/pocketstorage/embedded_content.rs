// Pocket Storage is one file on the registry: its four item models, the shared
// item texture, the lang file, the tier-1 recipe, the convention tags it names
// and the creative tab are the content mod in this bundle's data/
// (content/pocketstorage.hmod, built by tools/package.sh). It is written out
// once and loaded as a dev mod is, so it is on whenever this native mod is
// built in. An installed copy of the mod wins, as the first read. AE2 ships the
// same pattern.
fn pocketstorage_embedded_content(
    root: &std::path::Path,
    enabled: &std::collections::BTreeSet<String>,
    dev: &[std::path::PathBuf],
) -> core::ops::ControlFlow<std::path::PathBuf> {
    static CONTENT: &[u8] =
        horizon_client::native_data!("pocketstorage", "pocketstorage-content.hmod");
    let dir = std::env::current_dir()
        .unwrap_or_default()
        .join(".horizon-cache");
    let file = dir.join("pocketstorage-embedded-content.hmod");
    if dev.contains(&file) {
        return core::ops::ControlFlow::Continue(());
    }
    let same = std::fs::read(&file).is_ok_and(|on_disk| on_disk == CONTENT);
    if !same
        && std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::write(&file, CONTENT))
            .is_err()
    {
        log::warn!(
            "Horizon: Pocket Storage's content could not be written to {}",
            file.display()
        );
        return core::ops::ControlFlow::Continue(());
    }
    let mut with = dev.to_vec();
    with.push(file);
    core::ops::ControlFlow::Break(content_root(root, enabled, &with))
}

//! Options from outside: environment variables natively, and in the
//! browser the page's address after the `#`, so a link can start the stroll
//! (`…/#stroll`, `…/#stroll=tour&body=mars&stay=120`). There the name loses
//! its `MOON_` or `TRANQUILITY_` and is written small: `MOON_STROLL` is
//! `stroll`, `TRANQUILITY_BODY` is `body`.

/// The option's value, if it is given.
#[cfg(not(target_arch = "wasm32"))]
pub fn get(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

#[cfg(target_arch = "wasm32")]
pub fn get(name: &str) -> Option<String> {
    let hash = web_sys::window()?.location().hash().ok()?;
    let short = name.trim_start_matches("MOON_").trim_start_matches("TRANQUILITY_").to_lowercase();
    hash.trim_start_matches('#').split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=').unwrap_or((pair, "1"));
        (key == short).then(|| value.to_string())
    })
}

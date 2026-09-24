pub fn device_label(user_agent: Option<&str>) -> String {
    let Some(ua) = user_agent.map(|value| value.to_ascii_lowercase()) else {
        return "Unknown device".into();
    };
    let browser = if ua.contains("edg/") {
        "Edge"
    } else if ua.contains("chrome/") && !ua.contains("chromium") {
        "Chrome"
    } else if ua.contains("firefox/") {
        "Firefox"
    } else if ua.contains("safari/") && !ua.contains("chrome/") {
        "Safari"
    } else {
        "Browser"
    };
    let os = if ua.contains("iphone") || ua.contains("ipad") {
        "iOS"
    } else if ua.contains("android") {
        "Android"
    } else if ua.contains("mac os") {
        "macOS"
    } else if ua.contains("windows") {
        "Windows"
    } else if ua.contains("linux") {
        "Linux"
    } else {
        "Unknown OS"
    };
    format!("{browser} on {os}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_chrome_windows() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36";
        assert_eq!(device_label(Some(ua)), "Chrome on Windows");
    }
}

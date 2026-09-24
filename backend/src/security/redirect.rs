/// Allow only same-application relative paths used after sign-in.
/// Rejects scheme-relative and backslash tricks that become open redirects.
pub fn safe_return_to(value: &str) -> Option<String> {
    if value.len() > 2048
        || !value.starts_with('/')
        || value.starts_with("//")
        || value.contains('\\')
        || value.contains('#')
    {
        return None;
    }
    let (path, query) = value
        .split_once('?')
        .map(|(path, query)| (path, Some(query)))
        .unwrap_or((value, None));
    if path.contains('@') {
        return None;
    }
    let decoded_path = percent_decode(path);
    if decoded_path.contains('\\')
        || decoded_path.starts_with("//")
        || decoded_path.contains("://")
        || decoded_path.split('/').any(|segment| segment == "..")
    {
        return None;
    }
    let allowed = decoded_path == "/oauth/authorize"
        || decoded_path.starts_with("/account")
        || decoded_path.starts_with("/admin")
        || decoded_path == "/sign-in"
        || decoded_path == "/";
    if !allowed {
        return None;
    }
    if decoded_path == "/oauth/authorize" && query.is_none() {
        return None;
    }
    Some(value.to_string())
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(
                std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or(""),
                16,
            ) {
                out.push(hex);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn exact_redirect_allowed(registered: &[String], candidate: &str) -> bool {
    if candidate.len() > 2048
        || candidate
            .chars()
            .any(|c| c.is_control() || c.is_whitespace())
    {
        return false;
    }
    registered.iter().any(|uri| uri == candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_open_redirects() {
        assert!(safe_return_to("https://attacker.example").is_none());
        assert!(safe_return_to("//attacker.example").is_none());
        assert!(safe_return_to("/\\attacker.example").is_none());
        assert!(safe_return_to("/%2F%2Fattacker.example").is_none());
        assert!(safe_return_to("/oauth/authorize?client_id=knotree-study&redirect_uri=https%3A%2F%2Fstudy.knotree.com%2Fauth%2Fcallback").is_some());
        assert!(safe_return_to("/account/security").is_some());
        assert!(safe_return_to("/somewhere").is_none());
    }

    #[test]
    fn redirect_uri_is_exact() {
        let registered = vec!["https://study.knotree.com/auth/callback".into()];
        assert!(exact_redirect_allowed(
            &registered,
            "https://study.knotree.com/auth/callback"
        ));
        assert!(!exact_redirect_allowed(
            &registered,
            "https://study.knotree.com/auth/callback/extra"
        ));
        assert!(!exact_redirect_allowed(
            &registered,
            "https://evil.example/auth/callback"
        ));
    }
}

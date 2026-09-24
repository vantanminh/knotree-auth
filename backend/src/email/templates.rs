pub struct RenderedEmail {
    pub subject: String,
    pub text: String,
    pub html: String,
    pub template: &'static str,
}

fn layout(title: &str, body_html: &str, body_text: &str) -> (String, String) {
    let text =
        format!("{title}\n\n{body_text}\n\nKnotree Accounts\nhttps://accounts.knotree.com\n");
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
</head>
<body style="margin:0;padding:0;background:#f4f3ef;color:#1c1917;font-family:Georgia,'Iowan Old Style',serif;">
  <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="background:#f4f3ef;padding:32px 16px;">
    <tr><td align="center">
      <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="max-width:480px;background:#fffcf8;border:1px solid #e4e0d8;padding:32px 28px;">
        <tr><td style="font-family:'IBM Plex Sans',Helvetica,Arial,sans-serif;font-size:13px;letter-spacing:0.08em;text-transform:uppercase;color:#6b6560;padding-bottom:16px;">Knotree Accounts</td></tr>
        <tr><td style="font-family:'IBM Plex Sans',Helvetica,Arial,sans-serif;font-size:22px;line-height:1.3;padding-bottom:16px;">{title}</td></tr>
        <tr><td style="font-family:'IBM Plex Sans',Helvetica,Arial,sans-serif;font-size:15px;line-height:1.55;color:#1c1917;">{body_html}</td></tr>
      </table>
    </td></tr>
  </table>
</body>
</html>"#
    );
    (html, text)
}

fn esc(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn verification(link: &str) -> RenderedEmail {
    let title = "Verify your email";
    let text = format!(
        "Confirm this address to finish creating your Knotree account.\n\n{link}\n\nThis link expires in 24 hours and can be used once. If you did not create an account, you can ignore this email."
    );
    let html_body = format!(
        r#"<p>Confirm this address to finish creating your Knotree account.</p>
<p style="margin:24px 0;"><a href="{link}" style="display:inline-block;background:#1f3d32;color:#fffcf8;text-decoration:none;padding:10px 16px;border-radius:6px;">Verify email</a></p>
<p style="color:#6b6560;font-size:13px;">This link expires in 24 hours and can be used once. If you did not create an account, you can ignore this email.</p>"#,
        link = esc(link)
    );
    let (html, text) = layout(title, &html_body, &text);
    RenderedEmail {
        subject: title.into(),
        text,
        html,
        template: "verify-email",
    }
}

pub fn mfa_code(code: &str, minutes: i64) -> RenderedEmail {
    let title = "Your verification code";
    let text = format!(
        "Your Knotree verification code is {code}.\n\nIt expires in {minutes} minutes and can be used once.\n\nDo not share this code. If you did not try to sign in, you can ignore this email."
    );
    let html_body = format!(
        r#"<p>Enter this code to continue signing in.</p>
<p style="font-size:28px;letter-spacing:0.28em;font-weight:600;margin:24px 0;">{code}</p>
<p style="color:#6b6560;font-size:13px;">It expires in {minutes} minutes and can be used once. Do not share this code. If you did not try to sign in, you can ignore this email.</p>"#
    );
    let (html, text) = layout(title, &html_body, &text);
    RenderedEmail {
        subject: title.into(),
        text,
        html,
        template: "mfa-code",
    }
}

pub fn password_reset(link: &str) -> RenderedEmail {
    let title = "Reset your password";
    let text = format!(
        "We received a request to reset your Knotree password.\n\n{link}\n\nThis link expires in 30 minutes and can be used once. If you did not request a reset, you can ignore this email."
    );
    let html_body = format!(
        r#"<p>We received a request to reset your Knotree password.</p>
<p style="margin:24px 0;"><a href="{link}" style="display:inline-block;background:#1f3d32;color:#fffcf8;text-decoration:none;padding:10px 16px;border-radius:6px;">Reset password</a></p>
<p style="color:#6b6560;font-size:13px;">This link expires in 30 minutes and can be used once. If you did not request a reset, you can ignore this email.</p>"#,
        link = esc(link)
    );
    let (html, text) = layout(title, &html_body, &text);
    RenderedEmail {
        subject: title.into(),
        text,
        html,
        template: "reset-password",
    }
}

pub fn new_login(device: &str, when: &str, ip: &str, security_url: &str) -> RenderedEmail {
    let title = "New sign-in to your Knotree account";
    let text = format!(
        "Device: {device}\nTime: {when}\nIP: {ip}\nApproximate location: not available\n\nIf this was you, no action is needed.\n\nIf this was not you, secure your account:\n{security_url}"
    );
    let html_body = format!(
        r#"<p>A new sign-in used a device we have not seen recently.</p>
<p style="margin:16px 0;line-height:1.6;">Device<br><strong>{device}</strong><br><br>Time<br><strong>{when}</strong><br><br>IP<br><strong>{ip}</strong><br><br>Approximate location<br><strong>Not available</strong></p>
<p>If this was you, no action is needed.</p>
<p style="margin:24px 0;"><a href="{url}" style="display:inline-block;background:#1f3d32;color:#fffcf8;text-decoration:none;padding:10px 16px;border-radius:6px;">Review security</a></p>
<p style="color:#6b6560;font-size:13px;">If this was not you, change your password and sign out other sessions.</p>"#,
        device = esc(device),
        when = esc(when),
        ip = esc(ip),
        url = esc(security_url)
    );
    let (html, text) = layout(title, &html_body, &text);
    RenderedEmail {
        subject: title.into(),
        text,
        html,
        template: "new-login",
    }
}

pub fn security_alert(summary: &str, security_url: &str) -> RenderedEmail {
    let title = "Security alert for your Knotree account";
    let text = format!("{summary}\n\nReview your account:\n{security_url}\n\nIf you made this change, no action is needed.");
    let html_body = format!(
        r#"<p>{summary}</p>
<p style="margin:24px 0;"><a href="{url}" style="display:inline-block;background:#1f3d32;color:#fffcf8;text-decoration:none;padding:10px 16px;border-radius:6px;">Review security</a></p>
<p style="color:#6b6560;font-size:13px;">If you made this change, no action is needed.</p>"#,
        summary = esc(summary),
        url = esc(security_url)
    );
    let (html, text) = layout(title, &html_body, &text);
    RenderedEmail {
        subject: title.into(),
        text,
        html,
        template: "security-alert",
    }
}

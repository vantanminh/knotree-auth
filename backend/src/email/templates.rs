use crate::i18n::Locale;

pub struct RenderedEmail {
    pub subject: String,
    pub text: String,
    pub html: String,
    pub template: &'static str,
}

fn layout(locale: Locale, title: &str, body_html: &str, body_text: &str) -> (String, String) {
    let lang = locale.as_str();
    let text =
        format!("{title}\n\n{body_text}\n\nKnotree Accounts\nhttps://accounts.knotree.com\n");
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="{lang}">
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

fn button(url: &str, label: &str) -> String {
    format!(
        r#"<p style="margin:24px 0;"><a href="{url}" style="display:inline-block;background:#1f3d32;color:#fffcf8;text-decoration:none;padding:10px 16px;border-radius:6px;">{label}</a></p>"#,
        url = esc(url),
        label = esc(label)
    )
}

fn note(text: &str) -> String {
    format!(
        r#"<p style="color:#6b6560;font-size:13px;">{}</p>"#,
        esc(text)
    )
}

fn finish(
    locale: Locale,
    template: &'static str,
    title: &str,
    html_body: &str,
    text: &str,
) -> RenderedEmail {
    let (html, text) = layout(locale, &esc(title), html_body, text);
    RenderedEmail {
        subject: title.into(),
        text,
        html,
        template,
    }
}

pub fn verification(locale: Locale, link: &str) -> RenderedEmail {
    let (title, intro, cta, expiry) = match locale {
        Locale::En => (
            "Verify your email",
            "Confirm this address to finish creating your Knotree account.",
            "Verify email",
            "This link expires in 24 hours and can be used once. If you did not create an account, you can ignore this email.",
        ),
        Locale::Vi => (
            "Xác minh email của bạn",
            "Xác nhận địa chỉ này để hoàn tất việc tạo tài khoản Knotree.",
            "Xác minh email",
            "Liên kết hết hạn sau 24 giờ và chỉ dùng được một lần. Nếu bạn không tạo tài khoản, hãy bỏ qua email này.",
        ),
    };
    let text = format!("{intro}\n\n{link}\n\n{expiry}");
    let html_body = format!(
        "<p>{}</p>\n{}\n{}",
        esc(intro),
        button(link, cta),
        note(expiry)
    );
    finish(locale, "verify-email", title, &html_body, &text)
}

pub fn mfa_code(locale: Locale, code: &str, minutes: i64) -> RenderedEmail {
    let (title, text, intro, footer) = match locale {
        Locale::En => (
            "Your verification code",
            format!("Your Knotree verification code is {code}.\n\nIt expires in {minutes} minutes and can be used once.\n\nDo not share this code. If you did not try to sign in, you can ignore this email."),
            "Enter this code to continue signing in.",
            format!("It expires in {minutes} minutes and can be used once. Do not share this code. If you did not try to sign in, you can ignore this email."),
        ),
        Locale::Vi => (
            "Mã xác minh của bạn",
            format!("Mã xác minh Knotree của bạn là {code}.\n\nMã hết hạn sau {minutes} phút và chỉ dùng được một lần.\n\nKhông chia sẻ mã này. Nếu bạn không đăng nhập, hãy bỏ qua email này."),
            "Nhập mã này để tiếp tục đăng nhập.",
            format!("Mã hết hạn sau {minutes} phút và chỉ dùng được một lần. Không chia sẻ mã này. Nếu bạn không đăng nhập, hãy bỏ qua email này."),
        ),
    };
    let html_body = format!(
        r#"<p>{intro}</p>
<p style="font-size:28px;letter-spacing:0.28em;font-weight:600;margin:24px 0;">{code}</p>
{footer}"#,
        intro = esc(intro),
        code = esc(code),
        footer = note(&footer)
    );
    finish(locale, "mfa-code", title, &html_body, &text)
}

pub fn password_reset(locale: Locale, link: &str) -> RenderedEmail {
    let (title, intro, cta, expiry) = match locale {
        Locale::En => (
            "Reset your password",
            "We received a request to reset your Knotree password.",
            "Reset password",
            "This link expires in 30 minutes and can be used once. If you did not request a reset, you can ignore this email.",
        ),
        Locale::Vi => (
            "Đặt lại mật khẩu",
            "Chúng tôi nhận được yêu cầu đặt lại mật khẩu Knotree của bạn.",
            "Đặt lại mật khẩu",
            "Liên kết hết hạn sau 30 phút và chỉ dùng được một lần. Nếu bạn không yêu cầu, hãy bỏ qua email này.",
        ),
    };
    let text = format!("{intro}\n\n{link}\n\n{expiry}");
    let html_body = format!(
        "<p>{}</p>\n{}\n{}",
        esc(intro),
        button(link, cta),
        note(expiry)
    );
    finish(locale, "reset-password", title, &html_body, &text)
}

pub fn new_login(
    locale: Locale,
    device: &str,
    when: &str,
    ip: &str,
    security_url: &str,
) -> RenderedEmail {
    struct L {
        title: &'static str,
        intro: &'static str,
        device: &'static str,
        time: &'static str,
        location: &'static str,
        unavailable: &'static str,
        was_you: &'static str,
        not_you: &'static str,
        not_you_text: &'static str,
        cta: &'static str,
    }
    let l = match locale {
        Locale::En => L {
            title: "New sign-in to your Knotree account",
            intro: "A new sign-in used a device we have not seen recently.",
            device: "Device",
            time: "Time",
            location: "Approximate location",
            unavailable: "Not available",
            was_you: "If this was you, no action is needed.",
            not_you: "If this was not you, change your password and sign out other sessions.",
            not_you_text: "If this was not you, secure your account:",
            cta: "Review security",
        },
        Locale::Vi => L {
            title: "Đăng nhập mới vào tài khoản Knotree của bạn",
            intro: "Có một lần đăng nhập mới từ thiết bị chúng tôi chưa thấy gần đây.",
            device: "Thiết bị",
            time: "Thời gian",
            location: "Vị trí ước tính",
            unavailable: "Không có",
            was_you: "Nếu đó là bạn, bạn không cần làm gì thêm.",
            not_you: "Nếu không phải bạn, hãy đổi mật khẩu và đăng xuất các phiên khác.",
            not_you_text: "Nếu không phải bạn, hãy bảo vệ tài khoản:",
            cta: "Xem bảo mật",
        },
    };
    let text = format!(
        "{}: {device}\n{}: {when}\nIP: {ip}\n{}: {}\n\n{}\n\n{}\n{security_url}",
        l.device, l.time, l.location, l.unavailable, l.was_you, l.not_you_text
    );
    let html_body = format!(
        r#"<p>{intro}</p>
<p style="margin:16px 0;line-height:1.6;">{ld}<br><strong>{device}</strong><br><br>{lt}<br><strong>{when}</strong><br><br>IP<br><strong>{ip}</strong><br><br>{ll}<br><strong>{un}</strong></p>
<p>{was_you}</p>
{button}
{not_you}"#,
        intro = esc(l.intro),
        ld = esc(l.device),
        lt = esc(l.time),
        ll = esc(l.location),
        un = esc(l.unavailable),
        device = esc(device),
        when = esc(when),
        ip = esc(ip),
        was_you = esc(l.was_you),
        button = button(security_url, l.cta),
        not_you = note(l.not_you),
    );
    finish(locale, "new-login", l.title, &html_body, &text)
}

/// Security-relevant account changes that trigger an alert email.
#[derive(Clone, Copy, Debug)]
pub enum SecurityAlert {
    EmailChanged,
    AuthenticatorEnabled,
    AuthenticatorDisabled,
    EmailCodesEnabled,
    EmailCodesDisabled,
    RecoveryCodesRegenerated,
    PasswordReset,
    PasswordChanged,
}

impl SecurityAlert {
    fn summary(self, locale: Locale) -> &'static str {
        use SecurityAlert::*;
        match (self, locale) {
            (EmailChanged, Locale::En) => "The email address on your Knotree account was changed.",
            (EmailChanged, Locale::Vi) => "Địa chỉ email của tài khoản Knotree của bạn đã được thay đổi.",
            (AuthenticatorEnabled, Locale::En) => "An authenticator app is now enabled on your Knotree account.",
            (AuthenticatorEnabled, Locale::Vi) => "Ứng dụng xác thực đã được bật cho tài khoản Knotree của bạn.",
            (AuthenticatorDisabled, Locale::En) => "The authenticator app was turned off on your Knotree account.",
            (AuthenticatorDisabled, Locale::Vi) => "Ứng dụng xác thực đã bị tắt trên tài khoản Knotree của bạn.",
            (EmailCodesEnabled, Locale::En) => "Email verification codes are now enabled on your Knotree account.",
            (EmailCodesEnabled, Locale::Vi) => "Mã xác minh qua email đã được bật cho tài khoản Knotree của bạn.",
            (EmailCodesDisabled, Locale::En) => "Email verification codes were turned off on your Knotree account.",
            (EmailCodesDisabled, Locale::Vi) => "Mã xác minh qua email đã bị tắt trên tài khoản Knotree của bạn.",
            (RecoveryCodesRegenerated, Locale::En) => "Recovery codes for your Knotree account were regenerated. Previous codes no longer work.",
            (RecoveryCodesRegenerated, Locale::Vi) => "Mã khôi phục của tài khoản Knotree đã được tạo lại. Các mã cũ không còn dùng được.",
            (PasswordReset, Locale::En) => "The password for your Knotree account was reset. Other sessions were signed out.",
            (PasswordReset, Locale::Vi) => "Mật khẩu tài khoản Knotree của bạn đã được đặt lại. Các phiên khác đã bị đăng xuất.",
            (PasswordChanged, Locale::En) => "The password for your Knotree account was changed.",
            (PasswordChanged, Locale::Vi) => "Mật khẩu tài khoản Knotree của bạn đã được thay đổi.",
        }
    }
}

pub fn security_alert(locale: Locale, alert: SecurityAlert, security_url: &str) -> RenderedEmail {
    let summary = alert.summary(locale);
    let (title, review, done) = match locale {
        Locale::En => (
            "Security alert for your Knotree account",
            "Review your account:",
            "If you made this change, no action is needed.",
        ),
        Locale::Vi => (
            "Cảnh báo bảo mật cho tài khoản Knotree của bạn",
            "Xem lại tài khoản:",
            "Nếu bạn đã thực hiện thay đổi này, bạn không cần làm gì thêm.",
        ),
    };
    let cta = match locale {
        Locale::En => "Review security",
        Locale::Vi => "Xem bảo mật",
    };
    let text = format!("{summary}\n\n{review}\n{security_url}\n\n{done}");
    let html_body = format!(
        "<p>{}</p>\n{}\n{}",
        esc(summary),
        button(security_url, cta),
        note(done)
    );
    finish(locale, "security-alert", title, &html_body, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_vietnamese_email() {
        let message = verification(Locale::Vi, "https://example.com/v?token=a&b");
        assert_eq!(message.subject, "Xác minh email của bạn");
        assert!(message.html.contains(r#"<html lang="vi">"#));
        assert!(message.html.contains("token=a&amp;b"));
        let alert = security_alert(Locale::En, SecurityAlert::PasswordChanged, "https://x");
        assert!(alert.text.contains("was changed"));
    }
}

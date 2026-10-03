//! Locale handling for user-facing text (API errors and email).
//!
//! Supported locales are English (default) and Vietnamese. A request's locale
//! comes from `Accept-Language`; a user's stored preference (`users.locale`)
//! wins for email so messages sent outside a request still match the user.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

tokio::task_local! {
    pub static REQUEST_LOCALE: Locale;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    #[default]
    En,
    Vi,
}

impl Locale {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Vi => "vi",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        let primary = value
            .trim()
            .split(['-', '_'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        match primary.as_str() {
            "en" => Some(Self::En),
            "vi" => Some(Self::Vi),
            _ => None,
        }
    }

    /// Picks the best supported locale from an `Accept-Language` header,
    /// honouring q-values. Falls back to English.
    pub fn from_accept_language(header: Option<&str>) -> Self {
        let Some(header) = header else {
            return Self::En;
        };
        let mut best: Option<(f32, Self)> = None;
        for part in header.split(',').take(16) {
            let mut pieces = part.split(';');
            let tag = pieces.next().unwrap_or_default();
            let q = pieces
                .find_map(|p| p.trim().strip_prefix("q="))
                .and_then(|q| q.trim().parse::<f32>().ok())
                .unwrap_or(1.0);
            if let Some(locale) = Self::parse(tag) {
                if q > 0.0 && best.is_none_or(|(bq, _)| q > bq) {
                    best = Some((q, locale));
                }
            }
        }
        best.map(|(_, l)| l).unwrap_or(Self::En)
    }

    /// Locale of the request being served, English outside a request.
    pub fn current() -> Self {
        REQUEST_LOCALE.try_with(|l| *l).unwrap_or_default()
    }
}

/// Stored locale for a user, falling back to English.
pub async fn user_locale(db: &PgPool, user_id: Uuid) -> Locale {
    sqlx::query_scalar::<_, String>("SELECT locale FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .and_then(|value| Locale::parse(&value))
        .unwrap_or_default()
}

/// Translates a built-in English message for the given locale. Unknown
/// messages are returned unchanged.
pub fn translate(message: &str, locale: Locale) -> &str {
    if locale == Locale::En {
        return message;
    }
    VI.iter()
        .find(|(en, _)| *en == message)
        .map(|(_, vi)| *vi)
        .unwrap_or(message)
}

const VI: &[(&str, &str)] = &[
    ("The username, email or password is incorrect.", "Tên người dùng, email hoặc mật khẩu không đúng."),
    ("Usernames are 3 to 39 letters, numbers or hyphens and cannot start or end with a hyphen.", "Tên người dùng dài 3 đến 39 ký tự gồm chữ, số hoặc dấu gạch ngang, không bắt đầu hay kết thúc bằng dấu gạch ngang."),
    ("That username is reserved.", "Tên người dùng này đã được giữ lại."),
    ("That username is already taken.", "Tên người dùng này đã có người dùng."),
    ("Could not pick a username. Try again.", "Không chọn được tên người dùng. Hãy thử lại."),
    ("You can change your username once every 30 days.", "Bạn chỉ có thể đổi tên người dùng 30 ngày một lần."),
    ("An account can have up to three email addresses.", "Mỗi tài khoản có tối đa ba địa chỉ email."),
    ("This email is already used by a Knotree account.", "Email này đã được một tài khoản Knotree sử dụng."),
    ("Verify this email before making it primary.", "Hãy xác minh email này trước khi đặt làm email chính."),
    ("Make another email primary before removing this one.", "Hãy đặt email khác làm email chính trước khi gỡ email này."),
    ("Too many attempts. Try again later.", "Bạn đã thử quá nhiều lần. Vui lòng thử lại sau."),
    ("Sign in to continue.", "Đăng nhập để tiếp tục."),
    ("Not found.", "Không tìm thấy."),
    ("This account is disabled.", "Tài khoản này đã bị vô hiệu hoá."),
    ("Reset your password to continue.", "Hãy đặt lại mật khẩu để tiếp tục."),
    ("The email could not be sent. Try again.", "Không gửi được email. Vui lòng thử lại."),
    ("Confirm your password again to continue.", "Xác nhận lại mật khẩu để tiếp tục."),
    ("Something went wrong.", "Đã có lỗi xảy ra."),
    ("Admin access is required.", "Cần quyền quản trị."),
    ("Authenticator is already enabled.", "Ứng dụng xác thực đã được bật."),
    ("Choose a less common password.", "Hãy chọn mật khẩu ít phổ biến hơn."),
    ("Choose a valid verification method.", "Hãy chọn phương thức xác minh hợp lệ."),
    ("Email codes are not enabled.", "Mã qua email chưa được bật."),
    ("Enter a name up to 80 characters.", "Nhập tên tối đa 80 ký tự."),
    ("Enter a valid email address.", "Nhập địa chỉ email hợp lệ."),
    ("Enter the 6-digit code.", "Nhập mã gồm 6 chữ số."),
    ("GitHub did not accept this sign-in.", "GitHub không chấp nhận lần đăng nhập này."),
    ("GitHub sign-in is not configured.", "Đăng nhập bằng GitHub chưa được cấu hình."),
    ("Google did not accept this sign-in.", "Google không chấp nhận lần đăng nhập này."),
    ("Google identity token is invalid.", "Mã định danh Google không hợp lệ."),
    ("Google sign-in is not configured.", "Đăng nhập bằng Google chưa được cấu hình."),
    ("No email is available.", "Không có email nào."),
    ("Password cannot match your email.", "Mật khẩu không được trùng với email."),
    ("Password is too long.", "Mật khẩu quá dài."),
    ("Passwords do not match.", "Mật khẩu không khớp."),
    ("Refresh the page and try again.", "Tải lại trang và thử lại."),
    ("Start authenticator setup again.", "Hãy bắt đầu lại việc thiết lập ứng dụng xác thực."),
    ("Status must be active or disabled.", "Trạng thái phải là hoạt động hoặc vô hiệu hoá."),
    ("That code is not valid.", "Mã không hợp lệ."),
    ("The provider did not share an email.", "Nhà cung cấp không chia sẻ email."),
    ("The request was denied.", "Yêu cầu đã bị từ chối."),
    ("This account has no password yet.", "Tài khoản này chưa có mật khẩu."),
    ("This account is pending deletion.", "Tài khoản này đang chờ xoá."),
    ("This authorization request expired.", "Yêu cầu uỷ quyền này đã hết hạn."),
    ("This authorization request is no longer valid.", "Yêu cầu uỷ quyền này không còn hợp lệ."),
    ("This origin is not allowed.", "Nguồn gốc này không được phép."),
    ("This reset link is invalid or expired.", "Liên kết đặt lại không hợp lệ hoặc đã hết hạn."),
    ("This sign-in attempt expired. Start again.", "Phiên đăng nhập này đã hết hạn. Hãy bắt đầu lại."),
    ("Use at least 10 characters.", "Dùng ít nhất 10 ký tự."),
    ("Verify your email first.", "Hãy xác minh email trước."),
    ("Add a password or another provider before disconnecting this one.", "Hãy thêm mật khẩu hoặc nhà cung cấp khác trước khi ngắt kết nối nhà cung cấp này."),
    ("Add an email before enabling an authenticator.", "Hãy thêm email trước khi bật ứng dụng xác thực."),
    ("Admin access requires an authenticator app.", "Quyền quản trị yêu cầu ứng dụng xác thực."),
    ("An account already uses this email. Sign in and connect the provider from security settings.", "Đã có tài khoản dùng email này. Hãy đăng nhập và kết nối nhà cung cấp trong phần cài đặt bảo mật."),
    ("An account with this email already exists.", "Đã có tài khoản dùng email này."),
    ("Confirm with an authenticator or recovery code.", "Xác nhận bằng ứng dụng xác thực hoặc mã khôi phục."),
    ("Enable an authenticator before generating recovery codes.", "Hãy bật ứng dụng xác thực trước khi tạo mã khôi phục."),
    ("GitHub did not provide a verified email.", "GitHub không cung cấp email đã xác minh."),
    ("Google did not return an identity token.", "Google không trả về mã định danh."),
    ("Name contains unsupported characters.", "Tên chứa ký tự không được hỗ trợ."),
    ("Only active accounts can be asked to reset a password.", "Chỉ tài khoản đang hoạt động mới có thể được yêu cầu đặt lại mật khẩu."),
    ("Password contains unsupported characters.", "Mật khẩu chứa ký tự không được hỗ trợ."),
    ("That provider account is already connected to another user.", "Tài khoản nhà cung cấp này đã được kết nối với người dùng khác."),
    ("That provider account is already connected.", "Tài khoản nhà cung cấp này đã được kết nối."),
    ("The admin account cannot be deleted here.", "Không thể xoá tài khoản quản trị tại đây."),
    ("The admin account must keep an authenticator enabled.", "Tài khoản quản trị phải luôn bật ứng dụng xác thực."),
    ("The provider did not verify this email.", "Nhà cung cấp chưa xác minh email này."),
    ("This identity provider is not available.", "Nhà cung cấp định danh này không khả dụng."),
    ("This verification link is invalid or expired.", "Liên kết xác minh không hợp lệ hoặc đã hết hạn."),
    ("Verify your email before using email codes.", "Hãy xác minh email trước khi dùng mã qua email."),
    ("You cannot disable your own admin account.", "Bạn không thể vô hiệu hoá tài khoản quản trị của chính mình."),
    ("Choose a supported language.", "Hãy chọn ngôn ngữ được hỗ trợ."),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_accept_language() {
        assert_eq!(Locale::from_accept_language(None), Locale::En);
        assert_eq!(
            Locale::from_accept_language(Some("vi-VN,vi;q=0.9,en;q=0.8")),
            Locale::Vi
        );
        assert_eq!(
            Locale::from_accept_language(Some("en-US,vi;q=0.5")),
            Locale::En
        );
        assert_eq!(
            Locale::from_accept_language(Some("fr,vi;q=0.4")),
            Locale::Vi
        );
        assert_eq!(Locale::from_accept_language(Some("fr")), Locale::En);
    }

    #[test]
    fn translates_known_messages() {
        assert_eq!(
            translate("Passwords do not match.", Locale::Vi),
            "Mật khẩu không khớp."
        );
        assert_eq!(
            translate("Passwords do not match.", Locale::En),
            "Passwords do not match."
        );
        assert_eq!(translate("unknown", Locale::Vi), "unknown");
    }
}

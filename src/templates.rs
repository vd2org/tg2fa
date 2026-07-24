use crate::auth_service::AuthRequest;
use crate::geoip_service::GeoipResult;
use tgrs::{
    ButtonStyle, EditMessageReplyMarkup, InlineKeyboardButton, InlineKeyboardMarkup, LinkPreviewOptions, ParseMode,
    SendMessage,
};

use minijinja::{context, Environment};

static NEW_REQUEST_T: &str = "new_request";
static NEW_REQUEST_T_SRC: &str = r#"
<b>New login request!</b>

Username: <code>{{ username }}</code>
IP: <code>{{ ip_addr }}</code>
Device: <code>{{ device }}</code>
Location: {{ country_code | country_code_to_flag }}<code>{{ country }}</code>, <code>{{ city }}</code>
Organization: <code>{{ organization }}</code>

<i>Click 'Allow' to approve or 'Deny' to reject this request.</i>"#;
static NEW_REQUEST_BTN_ALLOW_T: &str = "new_request_btn_allow";
static NEW_REQUEST_BTN_ALLOW_T_SRC: &str = r#"Allow"#;
static NEW_REQUEST_BTN_DENY_T: &str = "new_request_btn_deny";
static NEW_REQUEST_BTN_DENY_T_SRC: &str = r#"Deny"#;

static UPDATE_REQUEST_BTN_ALLOW_T: &str = "update_request_btn_allow";
static UPDATE_REQUEST_BTN_ALLOW_T_SRC: &str = r#"✅ Allowed"#;
static UPDATE_REQUEST_BTN_DENY_T: &str = "update_request_btn_deny";
static UPDATE_REQUEST_BTN_DENY_T_SRC: &str = r#"⛔️ Denied"#;
static UPDATE_REQUEST_EXPIRED_T: &str = "update_request_expired";
static UPDATE_REQUEST_EXPIRED_T_SRC: &str = r#"😥 Expired"#;
static UPDATE_REQUEST_CODE_T: &str = "update_request_code";
static UPDATE_REQUEST_CODE_T_SRC: &str = r#"{{ code | chunk_and_join(3, "➖") | digits_to_emojis }}"#;

static USER_ID_T: &str = "user_id";
static USER_ID_T_SRC: &str = r#"🔸 Your USERID is: <code>{{ chat_id }}</code>"#;

const DISABLE_LINK_PREVIEW: LinkPreviewOptions = LinkPreviewOptions { is_disabled: Some(true) };

pub enum Resolution {
    Allow(String),
    Deny,
    Expired,
}

#[derive(Debug, Clone)]
pub struct Templates {
    env: Environment<'static>,
}

impl Templates {
    pub fn new() -> Self {
        let mut env = Environment::new();
        env.add_filter("country_code_to_flag", Self::country_code_to_flag);
        env.add_filter("digits_to_emojis", Self::digits_to_emojis);
        env.add_filter("chunk_and_join", Self::chunk_and_join);
        env.add_template(NEW_REQUEST_T, NEW_REQUEST_T_SRC.trim()).unwrap();
        env.add_template(NEW_REQUEST_BTN_ALLOW_T, NEW_REQUEST_BTN_ALLOW_T_SRC.trim()).unwrap();
        env.add_template(NEW_REQUEST_BTN_DENY_T, NEW_REQUEST_BTN_DENY_T_SRC.trim()).unwrap();
        env.add_template(UPDATE_REQUEST_BTN_ALLOW_T, UPDATE_REQUEST_BTN_ALLOW_T_SRC.trim()).unwrap();
        env.add_template(UPDATE_REQUEST_BTN_DENY_T, UPDATE_REQUEST_BTN_DENY_T_SRC.trim()).unwrap();
        env.add_template(UPDATE_REQUEST_EXPIRED_T, UPDATE_REQUEST_EXPIRED_T_SRC.trim()).unwrap();
        env.add_template(UPDATE_REQUEST_CODE_T, UPDATE_REQUEST_CODE_T_SRC.trim()).unwrap();
        env.add_template(USER_ID_T, USER_ID_T_SRC.trim()).unwrap();
        Templates { env }
    }

    fn country_code_to_flag(input: &str) -> String {
        //Offset to convert ASCII uppercase letters to Regional Indicator Symbols
        static OFFSET: u32 = '\u{1F1E6}' as u32 - 'A' as u32;
        input
            .chars()
            .map(|ch| {
                if ch.is_ascii_uppercase() {
                    // SAFETY: We have already checked that ch is an ASCII uppercase letter,
                    // so adding OFFSET will always produce a valid Unicode scalar value.
                    unsafe { char::from_u32_unchecked(ch as u32 + OFFSET) }
                } else {
                    ch
                }
            })
            .collect()
    }

    fn digits_to_emojis(input: &str) -> String {
        // Compose keycap emojis by combining ASCII digit + Variation Selector-16 + Combining Enclosing Keycap
        static VS16: char = '\u{FE0F}'; //Variation Selector-16
        static EK: char = '\u{20E3}'; //Enclosing Keycap

        input
            .chars()
            .flat_map(|ch| if ch.is_ascii_digit() { [Some(ch), Some(VS16), Some(EK)] } else { [Some(ch), None, None] })
            .flatten()
            .collect()
    }

    fn chunk_and_join(input: &str, chunk_size: usize, separator: &str) -> String {
        if chunk_size == 0 || input.is_empty() {
            return input.to_string();
        }

        input
            .chars()
            .collect::<Vec<char>>()
            .chunks(chunk_size)
            .map(|chunk| chunk.iter().collect::<String>())
            .collect::<Vec<String>>()
            .join(separator)
    }

    fn render(&self, template_name: &str, context: minijinja::value::Value) -> String {
        let tmpl = self.env.get_template(template_name).expect("Template not found");
        tmpl.render(context).expect("Failed to render template")
    }

    pub fn new_request(&self, req: &AuthRequest, geo_ip: &GeoipResult) -> SendMessage {
        let context = context!(
            username => req.meta_data.username, ip_addr => req.meta_data.ip_addr,
            device => req.meta_data.device_name, country_code => geo_ip.country_code, country => geo_ip.country, city => geo_ip.city, organization => geo_ip.org);
        let text = self.render("new_request", context);
        let allow_btn_text = self.render("new_request_btn_allow", context!());
        let deny_btn_text = self.render("new_request_btn_deny", context!());

        let allow_btn = InlineKeyboardButton::builder()
            .text(allow_btn_text)
            .callback_data("allow")
            .style(ButtonStyle::Success)
            .build();
        let deny_btn = InlineKeyboardButton::builder()
            .text(deny_btn_text)
            .callback_data("deny")
            .style(ButtonStyle::Danger)
            .build();

        let markup = InlineKeyboardMarkup::row([allow_btn, deny_btn]);

        SendMessage::builder()
            .chat_id(req.second_factor_data.id)
            .text(text)
            .link_preview_options(DISABLE_LINK_PREVIEW)
            .parse_mode(ParseMode::HTML)
            .reply_markup(markup.into())
            .build()
    }

    pub fn update_request(&self, chat_id: i64, message_id: u64, resolution: &Resolution) -> EditMessageReplyMarkup {
        let resolution_btn = InlineKeyboardButton::builder().callback_data("noop");

        // Render static texts from templates to keep all user-facing strings in one place
        let allow_text = self.render(UPDATE_REQUEST_BTN_ALLOW_T, context!());
        let deny_text = self.render(UPDATE_REQUEST_BTN_DENY_T, context!());
        let expired_text = self.render(UPDATE_REQUEST_EXPIRED_T, context!());

        let markup = match resolution {
            Resolution::Allow(code) => {
                let code_text = self.render(UPDATE_REQUEST_CODE_T, context!(code => code));
                let code_btn = InlineKeyboardButton::builder().text(code_text).copy_text(code.into()).build();
                let resolution_btn = resolution_btn.text(allow_text).build();
                InlineKeyboardMarkup::column([code_btn, resolution_btn])
            }
            Resolution::Deny => {
                let resolution_btn = resolution_btn.text(deny_text).build();
                InlineKeyboardMarkup::row([resolution_btn])
            }
            Resolution::Expired => {
                let resolution_btn = resolution_btn.text(expired_text).build();
                InlineKeyboardMarkup::row([resolution_btn])
            }
        };

        EditMessageReplyMarkup::builder().chat_id(chat_id).message_id(message_id).reply_markup(markup).build()
    }

    pub fn user_id(&self, chat_id: i64) -> SendMessage {
        let context = context!(chat_id => chat_id);
        let text = self.render(USER_ID_T, context);
        SendMessage::builder()
            .chat_id(chat_id)
            .text(text)
            .link_preview_options(DISABLE_LINK_PREVIEW)
            .parse_mode(ParseMode::HTML)
            .build()
    }
}

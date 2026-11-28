//! One CEF request context per profile (ADR 0004): its own cookies, cache,
//! site data, permissions and extensions under `<root>/<profile-id>/`.
//!
//! Each context is set up before it is reported ready (Phase 9):
//! certificate exceptions from earlier runs are cleared (a proceed lasts
//! until quit), and Chrome's own history recording is off (`profile.db`
//! keeps the profile's history). Popups stay Chrome's decision; the sites
//! always allowed them are Chrome's popup site settings.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use aeri_core::ids::ProfileId;
use cef::rc::Rc as _;
use cef::{
    CefString, CompletionCallback, ContentSettingTypes, ContentSettingValues,
    ImplCompletionCallback, ImplPreferenceManager, ImplPreferenceObserver, ImplRequestContext,
    ImplRequestContextHandler, JsonWriterOptions, PreferenceObserver, Registration, RequestContext,
    RequestContextHandler, RequestContextSettings, WrapCompletionCallback, WrapPreferenceObserver,
    WrapRequestContextHandler, request_context_create_context, wrap_completion_callback,
    wrap_preference_observer, wrap_request_context_handler, write_json,
};

use crate::Error;
use crate::extensions::{self, Extension};
use crate::preferences::{self, PrefSet, PrefValue, set as set_preference};

#[derive(Clone)]
pub struct ProfileContext {
    pub(crate) context: RequestContext,
    path: PathBuf,
}

/// Keeps an extension watch alive; dropping it stops the notifications.
pub struct ExtensionsWatch {
    _registration: Registration,
}

impl ProfileContext {
    /// Creates (or reopens) the profile's storage. CEF requires the path to
    /// be a direct child of the runtime's `root_cache_path`.
    ///
    /// The context initializes asynchronously and browsers cannot be created
    /// in it before then: `on_initialized` runs on the UI thread when it is
    /// ready.
    pub fn open(
        root_cache_path: &Path,
        profile: ProfileId,
        on_initialized: impl Fn() + 'static,
    ) -> Result<Self, Error> {
        let path = root_cache_path.join(profile.to_string());
        std::fs::create_dir_all(&path).map_err(|e| Error::Io(e.to_string()))?;
        // Every Aeri tab is a Chrome window, so Chrome's session service
        // records them; reopening the profile, Chrome restores them as
        // windows no Aeri tab owns (blank, ordered in, a renderer each),
        // and they are recorded again. They piled up across launches
        // (Observed: 63 windows, 70 renderers for 18 tabs). Aeri restores
        // its own session; Chrome's is dropped before Chrome reads it.
        match std::fs::remove_dir_all(path.join("Sessions")) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => tracing::warn!(%error, "could not drop Chrome's session"),
        }
        let settings = RequestContextSettings {
            cache_path: CefString::from(path.to_string_lossy().as_ref()),
            persist_session_cookies: 1,
            ..Default::default()
        };
        let mut handler = AeriRequestContextHandler::new(Rc::new(on_initialized));
        let context = request_context_create_context(Some(&settings), Some(&mut handler))
            .ok_or(Error::RequestContext)?;
        Ok(Self { context, path })
    }

    /// Whether Chrome lets `url`'s pages open popups without a gesture (the
    /// site's popup setting is Allow).
    pub fn popups_allowed(&self, url: &str) -> bool {
        popups_allowed(&self.context, url)
    }

    /// Sets `url`'s site to always allow popups, or back to Chrome's default
    /// (blocked without a gesture). Chrome's site settings show the choice.
    pub fn set_popups_allowed(&self, url: &str, allowed: bool) {
        let url = CefString::from(url);
        let value = if allowed {
            ContentSettingValues::ALLOW
        } else {
            ContentSettingValues::DEFAULT
        };
        self.context.set_content_setting(
            Some(&url),
            Some(&url),
            ContentSettingTypes::POPUPS,
            value,
        );
    }

    /// Sets one of the profile's boolean Chrome preferences. Main thread,
    /// after the context is initialized.
    pub fn set_bool_preference(&self, name: &str, value: bool) -> Result<(), String> {
        set_preference(&self.context, name, &PrefValue::Bool(value))
    }

    /// An integer preference (see [`crate::preferences`] for the names).
    pub fn set_int_preference(&self, name: &str, value: i32) -> Result<(), String> {
        set_preference(&self.context, name, &PrefValue::Int(value))
    }

    pub fn set_string_preference(&self, name: &str, value: &str) -> Result<(), String> {
        set_preference(&self.context, name, &PrefValue::String(value.to_owned()))
    }

    pub fn set_list_preference(&self, name: &str, value: Vec<PrefValue>) -> Result<(), String> {
        set_preference(&self.context, name, &PrefValue::List(value))
    }

    pub fn set_dict_preference(
        &self,
        name: &str,
        value: Vec<(String, PrefValue)>,
    ) -> Result<(), String> {
        set_preference(&self.context, name, &PrefValue::Dict(value))
    }

    /// Sets every preference in `prefs`, in order, on CEF's UI thread: at
    /// once when called from it, else posted. Chromium's refusals are logged
    /// (a posted task cannot report them).
    pub fn apply_preferences(&self, prefs: PrefSet) {
        preferences::apply_on_ui(self.context.clone(), prefs);
    }

    /// Installed, enabled extensions with a toolbar action. Main thread,
    /// after the context is initialized.
    pub fn extensions(&self) -> Vec<Extension> {
        let name = CefString::from(extensions::SETTINGS_PREFERENCE);
        let Some(mut settings) = self.context.preference(Some(&name)) else {
            return Vec::new();
        };
        let json = CefString::from(&write_json(Some(&mut settings), JsonWriterOptions::DEFAULT));
        match serde_json::from_str(&json.to_string()) {
            Ok(settings) => extensions::from_settings(&settings, &self.path),
            Err(error) => {
                tracing::warn!(%error, "unreadable extension settings");
                Vec::new()
            }
        }
    }

    /// Calls `on_change` on the main thread whenever extensions are
    /// installed, removed, enabled or disabled.
    pub fn watch_extensions(&self, on_change: impl Fn() + 'static) -> Option<ExtensionsWatch> {
        let name = CefString::from(extensions::SETTINGS_PREFERENCE);
        let mut observer = ExtensionsObserver::new(Rc::new(on_change));
        let registration = self
            .context
            .add_preference_observer(Some(&name), Some(&mut observer))?;
        Some(ExtensionsWatch {
            _registration: registration,
        })
    }
}

wrap_preference_observer! {
    struct ExtensionsObserver {
        on_change: Rc<dyn Fn()>,
    }

    impl PreferenceObserver {
        fn on_preference_changed(&self, _name: Option<&CefString>) {
            (self.on_change)();
        }
    }
}

wrap_request_context_handler! {
    struct AeriRequestContextHandler {
        on_initialized: Rc<dyn Fn()>,
    }

    impl RequestContextHandler {
        fn on_request_context_initialized(&self, request_context: Option<&mut RequestContext>) {
            let Some(context) = request_context else {
                return;
            };
            // Aeri's `profile.db` history is the profile's only one, so
            // clearing it clears everything the profile recorded.
            if let Err(error) = set_preference(context, "history.saving_disabled", &PrefValue::Bool(true)) {
                tracing::warn!(error, "Chrome's history recording stays on");
            }
            // Popups need the Person's gesture unless a site is allowed; an
            // allow-everything default (set by Phase 9's first build, or in
            // Chrome's settings) goes back to Chrome's default.
            if context.content_setting(None, None, ContentSettingTypes::POPUPS)
                == ContentSettingValues::ALLOW
            {
                tracing::info!("default popup setting reset from Allow");
                context.set_content_setting(
                    None,
                    None,
                    ContentSettingTypes::POPUPS,
                    ContentSettingValues::DEFAULT,
                );
            }
            // Ready only once earlier runs' proceeds are gone, so no page of
            // this profile can load past a warning without a new click.
            let mut done = CertificatesCleared::new(self.on_initialized.clone());
            context.clear_certificate_exceptions(Some(&mut done));
        }
    }
}

wrap_completion_callback! {
    struct CertificatesCleared {
        on_initialized: Rc<dyn Fn()>,
    }

    impl CompletionCallback {
        fn on_complete(&self) {
            tracing::debug!("certificate exceptions cleared");
            (self.on_initialized)();
        }
    }
}

/// Whether the site setting lets `url`'s pages open popups without a gesture.
pub(crate) fn popups_allowed(context: &RequestContext, url: &str) -> bool {
    let url = CefString::from(url);
    context.content_setting(Some(&url), Some(&url), ContentSettingTypes::POPUPS)
        == ContentSettingValues::ALLOW
}

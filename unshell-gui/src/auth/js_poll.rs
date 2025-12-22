use serde::de::DeserializeOwned;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::js_sys::Promise;

pub struct PromiseWrapper<T> {
    state: Rc<RefCell<PromiseState<T>>>,
}

enum PromiseState<T> {
    Pending,
    Resolved(T),
    Rejected(String),
}

impl<T: 'static> PromiseWrapper<T>
where
    T: DeserializeOwned,
{
    /// Create a new PromiseWrapper from a JavaScript Promise
    /// The promise should resolve to a Response object (from fetch)
    pub fn new(promise: Promise) -> Self {
        let state = Rc::new(RefCell::new(PromiseState::Pending));
        let state_clone = state.clone();
        let state_clone2 = state.clone();

        // Success callback
        let success = Closure::once(move |value: JsValue| {
            if let Ok(response) = value.dyn_into::<web_sys::Response>() {
                if let Ok(json_promise) = response.json() {
                    let state_inner = state_clone.clone();

                    let json_success = Closure::once(move |json_value: JsValue| {
                        *state_inner.borrow_mut() = if let Some(body) = json_value.as_string() {
                            match serde_json::from_str(&body) {
                                Ok(data) => PromiseState::Resolved(data),
                                Err(e) => PromiseState::Rejected(format!("{:?}", e)),
                            }
                        } else {
                            PromiseState::Rejected(format!("Response was not a string"))
                        };

                        match serde_wasm_bindgen::from_value::<T>(json_value) {
                            Ok(data) => *state_inner.borrow_mut() = PromiseState::Resolved(data),
                            Err(e) => {
                                *state_inner.borrow_mut() =
                                    PromiseState::Rejected(format!("{:?}", e))
                            }
                        }
                    });

                    let _ = json_promise.then(&json_success);
                    json_success.forget();
                }
            }
        });

        // Error callback
        let error = Closure::once(move |err: JsValue| {
            *state_clone2.borrow_mut() = PromiseState::Rejected(
                err.as_string()
                    .unwrap_or_else(|| "Unknown error".to_string()),
            );
        });

        let _ = promise.then2(&success, &error);
        success.forget();
        error.forget();

        Self { state }
    }

    /// Poll the promise to check if it has completed
    /// Returns Some(T) if resolved successfully, None if still pending or rejected
    /// This is a lightweight check that's safe to call every frame
    #[inline]
    pub fn poll(&self) -> Option<T>
    where
        T: Clone,
    {
        match &*self.state.borrow() {
            PromiseState::Resolved(value) => Some(value.clone()),
            _ => None,
        }
    }

    /// Check if the promise is still pending (lightweight)
    #[inline]
    pub fn is_pending(&self) -> bool {
        matches!(&*self.state.borrow(), PromiseState::Pending)
    }

    /// Check if the promise was rejected (lightweight)
    #[inline]
    pub fn is_rejected(&self) -> bool {
        matches!(&*self.state.borrow(), PromiseState::Rejected(_))
    }

    /// Get the error message if rejected
    pub fn error(&self) -> Option<String> {
        match &*self.state.borrow() {
            PromiseState::Rejected(err) => Some(err.clone()),
            _ => None,
        }
    }
}

impl Auth {
    // pub fn get_test(url: String) -> Promise {
    //     let fut = Self::get_async(&url);

    //     // wasm_bindgen_futures::(fut)
    // }

    // pub fn get_async<R>(&self, url: &str) -> PromiseWrapper<R>
    // where
    //     // F: FnOnce(R) + 'static,
    //     R: DeserializeOwned + 'static,
    // {
    //     let token = self.token.as_ref().unwrap();

    //     let opts = RequestInit::new();
    //     opts.set_method("GET");

    //     let request = Request::new_with_str_and_init(url, &opts).unwrap();

    //     let headers = request.headers();

    //     headers.set("content-type", "application/json").unwrap();

    //     headers
    //         .set("Authorization", &format!("Bearer {}", token.token))
    //         .unwrap();

    //     let window = web_sys::window().unwrap();
    //     let promise = window.fetch_with_request(&request);

    //     // wasm_bindgen_futures::spawn_local(async move {
    //     //     let resp_value = JsFuture::from(window.fetch_with_request(&request))
    //     //         .await
    //     //         .unwrap();

    //     //     // `resp_value` is a `Response` object.
    //     //     assert!(resp_value.is_instance_of::<Response>());
    //     //     let resp: Response = resp_value.dyn_into().unwrap();

    //     //     // Convert this other `Promise` into a rust `Future`.
    //     //     if let Ok(json) = resp.json() {
    //     //         if let Ok(json) = JsFuture::from(json).await {
    //     //             crate::log(&format!("{json:?}"));
    //     //             // let json = ;
    //     //         }
    //     //     }

    //     //     // crate::log(text);
    //     //     // Any follow-up work here
    //     // });

    //     PromiseWrapper::new(promise)

    //     // Ok(())
    // }

    // pub fn get_async_callback<R, F>(&self, url: &str, callback: F) -> Result<()>
    // where
    //     F: FnOnce(R) + 'static,
    //     R: DeserializeOwned + 'static,
    // {
    //     if self.token.is_none() {
    //         return Err(ModuleError::Error("Not authenticated".into()));
    //     }

    //     let token = self.token.clone().unwrap();

    //     let url = url.to_string();

    //     let state_clone = self.auth_state.clone();

    //     wasm_bindgen_futures::future_to_promise(async move {
    //         let result = Self::get_async(&url, &token).await;

    //         match result {
    //             Ok(result) => callback(result),
    //             Err(err) => (*state_clone.lock()) = AuthState::Error(err.into()),
    //         }

    //         Ok(JsValue::NULL)
    //     });

    //     Ok(())
    // }

    // async fn get_async<R>(url: &str, token: &Token) -> Result<R>
    // where
    //     R: DeserializeOwned + 'static,
    // {
    //     let res = reqwest::Client::new()
    //         .get(format!("http://localhost:8080{url}"))
    //         .bearer_auth(&token.token)
    //         .send()
    //         .await
    //         .map_err(|e| ModuleError::Error(e.to_string()))?;

    //     match res.error_for_status() {
    //         Ok(res) => res
    //             .json()
    //             .await
    //             .map_err(|e| ModuleError::Error(e.to_string())),
    //         Err(err) => Err(ModuleError::Error(format!(
    //             "Server returned error: {err:?}"
    //         ))),
    //     }

    //     // .json::<R>()
    //     // .await
    //     // .map_err(|e| ModuleError::Error(e.to_string()))?;

    //     // serde_json::from_str(&res).map_err(|e| ModuleError::SerdeJsonError(e.to_string()))
    // }
}

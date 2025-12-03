use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    middleware,
    routing::{get, post},
};
use tokio::net::TcpListener;
use unshell_lib::{debug, info};

use crate::{
    api::{auth, structs::CurrentUser},
    server::Server,
};

pub async fn start_api(address: &str, server: Server) {
    let listener = TcpListener::bind(address)
        .await
        .expect("Unable to start listener");

    info!("Listening on {}", listener.local_addr().unwrap());

    let mut router = Router::new().route("/api/auth", post(auth::sign_in));
    router = route_get_trees(router);
    router = route_get_all_tree_values(router);
    router = route_get_tree_keys(router);
    router = route_trees(router);

    axum::serve(listener, router.with_state(server))
        .await
        .expect("Error serving application");
}

// Route the "keys" api for each tree
fn route_get_trees(router: Router<Server>) -> Router<Server> {
    router.route(
        "/api/trees",
        get(
            async |State(server): State<Server>, Extension(_): Extension<CurrentUser>| {
                debug!("GET /api/trees");
                let result = server.get_trees();

                Json(serde_json::to_value(result).unwrap())
            },
        )
        .layer(middleware::from_fn(auth::authorize)),
    )
}

// Route the "keys" api for each tree
fn route_get_tree_keys(router: Router<Server>) -> Router<Server> {
    router.route(
        "/api/keys/{*path}",
        get(
            async |State(server): State<Server>,
                   Path(path): Path<String>,
                   Extension(_): Extension<CurrentUser>| {
                debug!("GET /api/keys/{}", path);
                let result = server.get_keys(&path);

                Json(serde_json::to_value(result).unwrap())
            },
        )
        .layer(middleware::from_fn(auth::authorize)),
    )
}

// Route the "values" api to get all the values for each tree
fn route_get_all_tree_values(router: Router<Server>) -> Router<Server> {
    router.route(
        "/api/values/{*path}",
        get(
            async |State(server): State<Server>,
                   Path(path): Path<String>,
                   Extension(_): Extension<CurrentUser>| {
                debug!("GET /api/values/{}", path);
                let result = server.all_tree_values(&path);

                Json(serde_json::to_value(result).unwrap())
            },
        )
        .layer(middleware::from_fn(auth::authorize)),
    )
}

// Loop through all trees and add /api/<tree>/<path> POST aand GET listeners for them
fn route_trees(mut router: Router<Server>) -> Router<Server> {
    for tree in crate::DATABASE_TREES.iter() {
        router = router
            // Route GET requests to this tree
            .route(
                &format!("/api/{}/{{*path}}", tree),
                get(
                    async |State(server): State<Server>,
                           Path(path): Path<String>,
                           Extension(_): Extension<CurrentUser>| {
                        let result = server.get_value(tree, &path);
                        debug!("GET /api/{}/{}", tree.to_string(), path);

                        Json(serde_json::to_value(result).unwrap())
                    },
                )
                .layer(middleware::from_fn(auth::authorize)),
            )
            // Route POST requests to this tree
            .route(
                &format!("/api/{}/{{*path}}", tree),
                post(
                    async |State(server): State<Server>,
                           Path(path): Path<String>,
                           Extension(_): Extension<CurrentUser>,
                           body: String| {
                        let result = server.put_value(tree, &path, &body);
                        debug!("POST /api/{}/{}", tree.to_string(), path);

                        Json(serde_json::to_value(result).unwrap())
                    },
                )
                .layer(middleware::from_fn(auth::authorize)),
            );
    }
    router
}

use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    middleware,
    routing::{get, post},
};
use tokio::net::TcpListener;
use unshell_lib::{debug, info};

// axum_extra::

use crate::{auth, auth::structs::CurrentUser, logger::Logger, server::Server};

macro_rules! route_get {
    ($router:expr, $path:expr, $func:expr) => {{
        {
            $router.route(
                $path,
                get($func).layer(middleware::from_fn(auth::authorize)),
            )
        }
    }};
}

macro_rules! route_post {
    ($router:expr, $path:expr, $func:expr) => {{
        {
            $router.route(
                $path,
                post($func).layer(middleware::from_fn(auth::authorize)),
            )
        }
    }};
}

pub async fn start_api(address: &str, server: Server) {
    let listener = TcpListener::bind(address)
        .await
        .expect("Unable to start listener");

    info!("Listening on {}", listener.local_addr().unwrap());

    let mut router = Router::new().route("/api/auth", post(auth::sign_in));

    router = route_trees(router);

    router = route_get!(router, "/api/log/{*offset}", Logger::poll_logs_api);
    router = route_get!(router, "/api/trees", Server::get_trees_api);
    router = route_get!(router, "/api/keys/{*path}", Server::all_tree_keys_api);
    router = route_get!(router, "/api/values/{*path}", Server::all_tree_values_api);

    router = route_get!(router, "/api/interface/", Server::get_tree2_root);
    router = route_get!(router, "/api/interface/{*path}", Server::get_tree2);
    router = route_post!(router, "/api/interface/{*path}", Server::post_tree2);

    // router = route_get_log(router);

    axum::serve(listener, router.with_state(server))
        .await
        .expect("Error serving application");
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

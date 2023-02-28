use http_problem::prelude::StatusCode;

http_problem::define_custom_type! {
    type Conflict {
        type: "https://http.cat/409",
        title: "Conflict of keys",
        status: StatusCode::INTERNAL_SERVER_ERROR,
        detail(p): format!("Conflict in key {}", p.key),
        extensions: {
            key: String,
        }
    }
}

http_problem::define_custom_type! {
    type KeyNotFound {
        type: "https://http.cat/404",
        title: "Value not found in storage",
        status: StatusCode::INTERNAL_SERVER_ERROR,
        detail(p): format!("key not found  {}", p.key),
        extensions: {
            key: String,
        }
    }
}

http_problem::define_custom_type! {
    type Cardinality {
        type: "https://http.cat/400",
        title: "Values have conflicting ids",
        status: StatusCode::BAD_REQUEST,
        detail(p): format!("key not found  {}", p.key),
        extensions: {
            key: String,
        }
    }
}

http_problem::define_custom_type! {
    type Locking {
        type: "https://http.cat/500",
        title: "Error while locking value",
        status: StatusCode::BAD_REQUEST,
        detail(p): format!("Unexpected error while locking {}", p.message),
        extensions: {
            message: String
        }
    }
}

pub(crate) fn locking<E: std::fmt::Display>(err: E) -> Locking {
    Locking {
        message: err.to_string(),
    }
}

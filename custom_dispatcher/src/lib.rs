use std::{collections::HashMap, pin::Pin, sync::Arc};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

type Callback = Arc<
    dyn Fn(
            Vec<u8>,
        )
            -> Pin<Box<(dyn futures::Future<Output = ()> + std::marker::Send + std::marker::Sync)>>
        + 'static
        + std::marker::Send
        + std::marker::Sync,
>;

pub struct Listener {
    callback: Callback,
    limit: Option<u64>,
    _id: String,
}

#[derive(Default)]
pub struct EventEmitter {
    pub listeners: HashMap<String, Vec<Listener>>,
}

impl EventEmitter {
    pub fn new() -> Self {
        Self { ..Self::default() }
    }

    pub async fn on<FN, T>(&mut self, event: &str, callback: FN) -> String
    where
        for<'de> T: Deserialize<'de>,
        FN: Fn(
                T,
            ) -> Pin<
                Box<(dyn futures::Future<Output = ()> + std::marker::Send + std::marker::Sync)>,
            >
            + 'static
            + std::marker::Send
            + std::marker::Sync,
    {
        self.on_limited(event, None, callback).await
    }

    pub async fn emit<T>(&mut self, event: &str, value: T) -> Vec<tokio::task::JoinHandle<()>>
    where
        T: Serialize,
    {
        let mut callback_handlers: Vec<tokio::task::JoinHandle<()>> = Vec::new();

        if let Some(listeners) = self.listeners.get_mut(event) {
            let bytes: Vec<u8> = bincode::serialize(&value).unwrap();

            let mut listeners_to_remove: Vec<usize> = Vec::new();
            for (index, listener) in listeners.iter_mut().enumerate() {
                let cloned_bytes = bytes.clone();
                let callback = Arc::clone(&listener.callback);

                match listener.limit {
                    None => {
                        callback_handlers.push(tokio::spawn(Box::pin(async move {
                            callback(cloned_bytes).await;
                        })));
                    }
                    Some(limit) => {
                        if limit != 0 {
                            callback_handlers.push(tokio::spawn(Box::pin(async move {
                                callback(cloned_bytes).await;
                            })));
                        } else {
                            listeners_to_remove.push(index);
                        }
                    }
                }
            }

            for index in listeners_to_remove.into_iter().rev() {
                listeners.remove(index);
            }
        }

        callback_handlers
    }

    pub async fn on_limited<FN, T>(
        &mut self,
        event: &str,
        limit: Option<u64>,
        callback: FN,
    ) -> String
    where
        for<'de> T: Deserialize<'de>,
        FN: Fn(
                T,
            ) -> Pin<
                Box<(dyn futures::Future<Output = ()> + std::marker::Send + std::marker::Sync)>,
            >
            + 'static
            + std::marker::Send
            + std::marker::Sync,
    {
        let id = Uuid::new_v4().to_string();
        let parsed_callback = move |bytes: Vec<u8>| {
            let value: T = bincode::deserialize(&bytes).unwrap();
            callback(value)
        };

        let listener = Listener {
            _id: id.clone(),
            limit,
            callback: Arc::new(parsed_callback),
        };

        match self.listeners.get_mut(event) {
            Some(callbacks) => {
                callbacks.push(listener);
            }
            None => {
                self.listeners.insert(event.to_string(), vec![listener]);
            }
        }

        id
    }
}

use crate::service::{
    doc::edit::EditDocContext,
    ws::{entities::Socket, WsUser},
};
use async_stream::stream;
use flowy_document::protobuf::Revision;
use flowy_net::errors::{internal_error, Result as DocResult};
use futures::stream::StreamExt;
use std::sync::Arc;
use tokio::{
    sync::{mpsc, oneshot},
    task::spawn_blocking,
};

#[derive(Clone)]
pub struct EditUser {
    user: Arc<WsUser>,
    pub(crate) socket: Socket,
}

impl EditUser {
    pub fn id(&self) -> String { self.user.id().to_string() }
}

#[derive(Debug)]
pub enum EditMsg {
    Revision {
        user: Arc<WsUser>,
        socket: Socket,
        revision: Revision,
        ret: oneshot::Sender<DocResult<()>>,
    },
    DocumentJson {
        ret: oneshot::Sender<DocResult<String>>,
    },
}

pub struct EditDocActor {
    receiver: Option<mpsc::Receiver<EditMsg>>,
    edit_context: Arc<EditDocContext>,
}

impl EditDocActor {
    pub fn new(receiver: mpsc::Receiver<EditMsg>, edit_context: Arc<EditDocContext>) -> Self {
        Self {
            receiver: Some(receiver),
            edit_context,
        }
    }

    pub async fn run(mut self) {
        let mut receiver = self
            .receiver
            .take()
            .expect("DocActor's receiver should only take one time");

        let stream = stream! {
            loop {
                match receiver.recv().await {
                    Some(msg) => yield msg,
                    None => break,
                }
            }
        };
        stream.for_each(|msg| self.handle_message(msg)).await;
    }

    async fn handle_message(&self, msg: EditMsg) {
        match msg {
            EditMsg::Revision {
                user,
                socket,
                revision,
                ret,
            } => {
                // ret.send(self.handle_client_data(client_data, pool).await);
                let user = EditUser {
                    user: user.clone(),
                    socket: socket.clone(),
                };
                let _ = ret.send(self.edit_context.apply_revision(user, revision).await);
            },
            EditMsg::DocumentJson { ret } => {
                let edit_context = self.edit_context.clone();
                let json = spawn_blocking(move || edit_context.document_json())
                    .await
                    .map_err(internal_error);
                let _ = ret.send(json);
            },
        }
    }
}
use std::collections::HashSet;

use db_view::connection_form_window::{ConnectionFormWindow, ConnectionFormWindowConfig};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, AsyncApp, Context, ElementId, Entity, EventEmitter, FocusHandle,
    Focusable, FontWeight, InteractiveElement, IntoElement, KeyBinding, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, WeakEntity, Window, actions,
    div, px,
};
use gpui_component::button::{ButtonCustomVariant, ButtonVariant};
use gpui_component::menu::DropdownMenu;
use gpui_component::{
    ActiveTheme, Icon, IconName, InteractiveElementExt, Sizable, Size, WindowExt,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState},
    list::{List, ListState},
    menu::PopupMenuItem,
    popover::Popover,
    tooltip::Tooltip,
    v_flex,
};
use mongodb_view::{MongoFormWindow, MongoFormWindowConfig};
use one_core::connection_notifier::{ConnectionDataEvent, emit_connection_event, get_notifier};
use one_core::popup_window::{PopupWindowOptions, open_popup_window};
use one_core::storage::traits::Repository;
use one_core::storage::{
    ActiveConnections, ConnectionRepository, ConnectionType, DatabaseType, GlobalStorageState,
    RedisMode, StoredConnection, Workspace, WorkspaceRepository,
};
use one_core::tab_container::{TabContainer, TabContent, TabContentEvent};
use redis_view::{RedisFormWindow, RedisFormWindowConfig};
use rust_i18n::t;
use terminal_view::TerminalView;
use terminal_view::{SerialFormWindow, SerialFormWindowConfig};
use terminal_view::{SshFormWindow, SshFormWindowConfig};

use crate::home::home_connection_quick_open::ConnectionQuickOpenDelegate;
use crate::home::home_new_connection::NewConnectionDelegate;
use crate::home::home_strategy::build_connection_open_strategy;
use crate::home::home_workspace_filter::WorkspaceFilterDelegate;

actions!(home_tab, [OpenConnectionQuickOpen, NewConnectionShortcut]);

fn get_cached_team_options(_cx: &App) -> Vec<one_core::TeamOption> {
    Vec::new()
}

/// 检查用户是否可以编辑连接（简化版本，云同步移除后始终返回 true）
fn can_edit_connection(_conn: &StoredConnection, _cx: &App) -> bool {
    true
}

pub fn init(cx: &mut App) {
    cx.bind_keys([
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-o", OpenConnectionQuickOpen, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("alt-o", OpenConnectionQuickOpen, None),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-n", NewConnectionShortcut, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("alt-n", NewConnectionShortcut, None),
    ]);
}

// HomePage Entity - 管理 home 页面的所有状态

pub struct HomePage {
    focus_handle: FocusHandle,
    selected_filter: ConnectionType,
    pub(crate) workspaces: Vec<Workspace>,
    pub(crate) connections: Vec<StoredConnection>,
    pub(crate) tab_container: Entity<TabContainer>,
    search_input: Entity<InputState>,
    search_query: Entity<String>,
    pub(crate) editing_connection_id: Option<i64>,
    selected_connection_id: Option<i64>,
    editing_workspace_id: Option<i64>,
    pub(crate) filtered_workspace_ids: HashSet<i64>,
    pub(crate) workspace_filter_open: bool,
    workspace_filter_list: Option<Entity<ListState<WorkspaceFilterDelegate>>>,
    pub(crate) _subscriptions: Vec<Subscription>,
    pub(crate) terminal_views: Vec<WeakEntity<TerminalView>>,
}

impl HomePage {
    pub fn new(
        tab_container: Entity<TabContainer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_query = cx.new(|_| String::new());
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(t!("Home.search_placeholder"))
                .clean_on_escape()
        });

        // 订阅搜索输入变化
        let query_clone = search_query.clone();
        cx.subscribe_in(
            &search_input,
            window,
            move |_this, _input, event, _window, cx| {
                if let InputEvent::Change = event {
                    query_clone.update(cx, |q, cx| {
                        *q = _input.read(cx).text().to_string();
                        cx.notify();
                    });
                    cx.notify();
                }
            },
        )
        .detach();

        let mut page = Self {
            focus_handle: cx.focus_handle(),
            selected_filter: ConnectionType::All,
            workspaces: Vec::new(),
            connections: Vec::new(),
            tab_container,
            search_input,
            search_query,
            editing_connection_id: None,
            selected_connection_id: None,
            editing_workspace_id: None,
            filtered_workspace_ids: HashSet::new(),
            workspace_filter_open: false,
            workspace_filter_list: None,
            _subscriptions: Vec::new(),
            terminal_views: Vec::new(),
        };

        // 异步加载工作区
        page.load_workspaces(cx);

        // 加载连接
        page.load_connections(cx);

        // 订阅全局连接事件，当连接创建/更新时刷新列表
        if let Some(notifier) = get_notifier(cx) {
            cx.subscribe(
                &notifier,
                |this, _, event: &ConnectionDataEvent, cx| match event {
                    ConnectionDataEvent::ConnectionCreated { connection } => {
                        // 立即将新连接添加到列表，避免异步加载的时序问题
                        this.connections.push(connection.clone());
                        cx.notify();
                        // 然后异步重新加载以确保数据一致性
                        this.load_connections(cx);
                        // 云同步已禁用
                    }
                    ConnectionDataEvent::ConnectionUpdated { connection } => {
                        // 立即更新列表中的连接，避免异步加载的时序问题
                        if let Some(pos) =
                            this.connections.iter().position(|c| c.id == connection.id)
                        {
                            this.connections[pos] = connection.clone();
                        } else {
                            // 如果找不到，添加到列表
                            this.connections.push(connection.clone());
                        }
                        cx.notify();
                        // 然后异步重新加载以确保数据一致性
                        this.load_connections(cx);
                        // 云同步已禁用
                    }
                    ConnectionDataEvent::ConnectionDeleted { connection_id } => {
                        // 立即从列表中移除连接
                        this.connections.retain(|c| c.id != Some(*connection_id));
                        cx.notify();
                        // 然后异步重新加载以确保数据一致性
                        this.load_connections(cx);
                        // 云同步已禁用
                    }
                    ConnectionDataEvent::WorkspaceCreated { .. }
                    | ConnectionDataEvent::WorkspaceUpdated { .. }
                    | ConnectionDataEvent::WorkspaceDeleted { .. } => {
                        this.load_workspaces(cx);
                    }
                    ConnectionDataEvent::SchemaChanged { .. } => {
                        // SchemaChanged 由 db_tree_view 处理，此处无需操作
                    }
                },
            )
            .detach();
        }

        page
    }

    fn load_workspaces(&mut self, cx: &mut Context<Self>) {
        let storage = cx.global::<GlobalStorageState>().storage.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = (|| {
                let repo = storage
                    .get::<WorkspaceRepository>()
                    .ok_or_else(|| anyhow::anyhow!("WorkspaceRepository not found"))?;
                repo.list()
            })();

            match result {
                Ok(workspaces) => {
                    _ = this.update(cx, |this, cx| {
                        this.workspaces = workspaces;
                        cx.notify();
                    });
                }
                Err(e) => {
                    tracing::error!("Task join error: {}", e);
                }
            }
        })
        .detach();
    }

    fn load_connections(&mut self, cx: &mut Context<Self>) {
        let storage = cx.global::<GlobalStorageState>().storage.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = (|| {
                let repo = storage
                    .get::<ConnectionRepository>()
                    .ok_or_else(|| anyhow::anyhow!("ConnectionRepository not found"))?;
                repo.list()
            })();

            match result {
                Ok(connections) => {
                    _ = this.update(cx, |this, cx| {
                        this.connections = connections;
                        cx.notify();
                    });
                }
                Err(e) => {
                    tracing::error!("Task join error: {}", e);
                }
            }
        })
        .detach();
    }

    fn refresh_local_home_data(&mut self, cx: &mut Context<Self>) {
        self.load_workspaces(cx);
        self.load_connections(cx);
    }

    fn confirm_edit_connection(
        &mut self,
        conn_id: i64,
        conn_name: String,
        db_type: Option<DatabaseType>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let is_active = cx.global::<ActiveConnections>().is_active(conn_id);

        if is_active {
            window.open_dialog(cx, move |dialog, _window, _cx| {
                dialog
                    .title(t!("Connection.in_use_title").to_string().into_any_element())
                    .child(
                        t!("Connection.in_use_cannot_edit", conn_name = conn_name)
                            .to_string()
                            .into_any_element(),
                    )
                    .alert()
            });
        } else if let Some(db_type) = db_type {
            self.editing_connection_id = Some(conn_id);
            self.show_connection_form(db_type, window, cx);
        }
    }

    fn confirm_delete_connection(
        &mut self,
        conn_id: i64,
        conn_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let is_active = cx.global::<ActiveConnections>().is_active(conn_id);
        let view = cx.entity().clone();

        if is_active {
            window.open_dialog(cx, move |dialog, _window, _cx| {
                dialog
                    .title(t!("Connection.in_use_title").to_string().into_any_element())
                    .child(
                        t!("Connection.in_use_cannot_delete", conn_name = conn_name)
                            .to_string()
                            .into_any_element(),
                    )
                    .alert()
            });
        } else {
            window.open_dialog(cx, move |dialog, _window, _cx| {
                let view_clone = view.clone();
                dialog
                    .title(t!("Common.delete").to_string().into_any_element())
                    .child(
                        t!("Connection.delete_confirm", conn_name = conn_name)
                            .to_string()
                            .into_any_element(),
                    )
                    .confirm()
                    .on_ok(move |_, _, cx| {
                        let _ = view_clone.update(cx, |this, cx| {
                            this.delete_connection(conn_id, cx);
                        });
                        true
                    })
            });
        }
    }

    fn delete_connection(&mut self, conn_id: i64, cx: &mut Context<Self>) {
        let storage = cx.global::<GlobalStorageState>().storage.clone();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            // 删除本地连接
            let result = (|| {
                let repo = storage
                    .get::<ConnectionRepository>()
                    .ok_or_else(|| anyhow::anyhow!("ConnectionRepository not found"))?;
                repo.delete(conn_id)
            })();

            match result {
                Ok(_) => {
                    _ = this.update(cx, |this, cx| {
                        this.connections.retain(|c| c.id != Some(conn_id));
                        if this.selected_connection_id == Some(conn_id) {
                            this.selected_connection_id = None;
                        }
                        emit_connection_event(
                            ConnectionDataEvent::ConnectionDeleted {
                                connection_id: conn_id,
                            },
                            cx,
                        );
                        cx.notify();
                    });
                }
                Err(e) => {
                    tracing::error!("Failed to delete connection: {}", e);
                }
            }
        })
        .detach();
    }

    pub(crate) fn show_workspace_form(
        &mut self,
        workspace_id: Option<i64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let workspace_data =
            workspace_id.and_then(|id| self.workspaces.iter().find(|w| w.id == Some(id)).cloned());
        self.editing_workspace_id = workspace_id;
        let view = cx.entity().clone();
        let is_editing = workspace_id.is_some();
        let form = cx.new(|cx| {
            let mut input_state =
                InputState::new(window, cx).placeholder(t!("Workspace.name_placeholder"));
            if let Some(ref workspace) = workspace_data {
                input_state.set_value(workspace.name.clone(), window, cx);
            }
            input_state
        });

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let form_clone = form.clone();
            let view_clone = view.clone();
            let view_clone2 = view.clone();
            dialog
                .title(
                    (if is_editing {
                        t!("Workspace.edit")
                    } else {
                        t!("Workspace.new")
                    })
                    .to_string()
                    .into_any_element(),
                )
                .w(px(400.0))
                .child(Input::new(&form).size_full())
                .content_center()
                .confirm()
                .on_ok(move |_, _window, cx| {
                    let name = form_clone.read(cx).text().to_string();
                    if !name.is_empty() {
                        let _ = view_clone.update(cx, |this, cx| {
                            this.handle_save_workspace(name, cx);
                        });
                        true
                    } else {
                        false
                    }
                })
                .on_cancel(move |_, _, cx| {
                    let _ = view_clone2.update(cx, |this, _| {
                        this.editing_workspace_id = None;
                    });
                    true
                })
        });
    }

    pub(crate) fn show_connection_quick_open(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let parent = cx.entity();
        let connections = self.connections.clone();
        let list = cx.new(|cx| {
            let mut delegate = ConnectionQuickOpenDelegate::new(parent);
            delegate.update_items(&connections);
            ListState::new(delegate, window, cx).searchable(true)
        });

        let list_for_focus = list.clone();
        window.open_dialog(cx, move |dialog, _window, cx| {
            dialog
                .title("打开连接".to_string())
                .w(px(520.0))
                .child(
                    v_flex().gap_2().child(
                        List::new(&list)
                            .w_full()
                            .max_h(px(360.0))
                            .p(px(8.0))
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded(cx.theme().radius),
                    ),
                )
                .alert()
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text(t!("Common.close")),
                )
        });
        // 将焦点设置到 List 搜索框，使上下键和 Enter 键可用
        list_for_focus.update(cx, |state, cx| {
            state.focus(window, cx);
        });
    }

    pub(crate) fn show_new_connection_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let parent = cx.entity();
        let list = cx.new(|cx| {
            let delegate = NewConnectionDelegate::new(parent);
            ListState::new(delegate, window, cx).searchable(true)
        });

        let list_for_focus = list.clone();
        window.open_dialog(cx, move |dialog, _window, cx| {
            dialog
                .title(t!("Home.new_connection").to_string())
                .w(px(360.0))
                .child(
                    v_flex().gap_2().child(
                        List::new(&list)
                            .w_full()
                            .max_h(px(360.0))
                            .p(px(8.0))
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded(cx.theme().radius),
                    ),
                )
                .alert()
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text(t!("Common.close")),
                )
        });
        // 将焦点设置到 List 搜索框，使上下键和 Enter 键可用
        list_for_focus.update(cx, |state, cx| {
            state.focus(window, cx);
        });
    }

    pub(crate) fn open_connection_from_quick(
        &mut self,
        connection: &StoredConnection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let workspace = connection
            .workspace_id
            .and_then(|id| self.workspaces.iter().find(|w| w.id == Some(id)).cloned());
        let strategy = build_connection_open_strategy(connection.clone(), workspace);
        strategy.open(self, window, cx);
        cx.notify();
    }

    fn handle_save_workspace(&mut self, name: String, cx: &mut Context<Self>) {
        let storage = cx.global::<GlobalStorageState>().storage.clone();
        let editing_id = self.editing_workspace_id;

        let mut workspace = if let Some(id) = editing_id {
            // 编辑模式：从现有工作区更新
            let mut ws = self
                .workspaces
                .iter()
                .find(|w| w.id == Some(id))
                .cloned()
                .unwrap_or_else(|| Workspace::new(name.clone()));
            ws.name = name;
            ws
        } else {
            // 新建模式
            Workspace::new(name)
        };

        let result: anyhow::Result<Workspace> = (|| {
            let repo = storage
                .get::<WorkspaceRepository>()
                .ok_or_else(|| anyhow::anyhow!("WorkspaceRepository not found"))?;

            if editing_id.is_some() {
                repo.update(&mut workspace)?;
            } else {
                repo.insert(&mut workspace)?;
            }

            Ok(workspace)
        })();

        cx.spawn(async move |this, cx| match result {
            Ok(workspace) => {
                _ = this.update(cx, |this, cx| {
                    let workspace_id = workspace.id.unwrap_or(0);
                    if let Some(editing_id) = editing_id {
                        if let Some(pos) = this
                            .workspaces
                            .iter()
                            .position(|w| w.id == Some(editing_id))
                        {
                            this.workspaces[pos] = workspace;
                        }
                        emit_connection_event(
                            ConnectionDataEvent::WorkspaceUpdated { workspace_id },
                            cx,
                        );
                    } else {
                        this.workspaces.push(workspace);
                        emit_connection_event(
                            ConnectionDataEvent::WorkspaceCreated { workspace_id },
                            cx,
                        );
                    }
                    this.editing_workspace_id = None;
                    cx.notify();
                });
            }
            Err(e) => {
                tracing::error!("Failed to save workspace: {}", e);
            }
        })
        .detach();
    }

    pub(crate) fn delete_workspace(
        &mut self,
        workspace_id: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let workspace_name = self
            .workspaces
            .iter()
            .find(|w| w.id == Some(workspace_id))
            .map(|w| w.name.clone())
            .unwrap_or_default();

        let view = cx.entity().clone();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let view_clone = view.clone();
            dialog
                .title(t!("Workspace.delete").to_string().into_any_element())
                .child(
                    t!("Workspace.delete_confirm", workspace_name = workspace_name)
                        .to_string()
                        .into_any_element(),
                )
                .confirm()
                .on_ok(move |_, _window, cx| {
                    let _ = view_clone.update(cx, |this, cx| {
                        this.handle_delete_workspace(workspace_id, cx);
                    });
                    true
                })
        });
    }

    fn handle_delete_workspace(&mut self, workspace_id: i64, cx: &mut Context<Self>) {
        let storage = cx.global::<GlobalStorageState>().storage.clone();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            // 删除本地工作空间
            let result = (|| {
                let repo = storage
                    .get::<WorkspaceRepository>()
                    .ok_or_else(|| anyhow::anyhow!("WorkspaceRepository not found"))?;
                repo.delete(workspace_id)
            })();

            match result {
                Ok(_) => {
                    _ = this.update(cx, |this, cx| {
                        this.workspaces.retain(|w| w.id != Some(workspace_id));
                        this.filtered_workspace_ids.remove(&workspace_id);
                        emit_connection_event(
                            ConnectionDataEvent::WorkspaceDeleted { workspace_id },
                            cx,
                        );
                        cx.notify();
                    });
                }
                Err(e) => {
                    tracing::error!("Failed to delete workspace: {}", e);
                }
            }
        })
        .detach();
    }

    pub(crate) fn show_connection_form(
        &mut self,
        db_type: DatabaseType,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editing_conn = self
            .editing_connection_id
            .and_then(|id| self.connections.iter().find(|c| c.id == Some(id)).cloned());

        let config = ConnectionFormWindowConfig {
            db_type,
            editing_connection: editing_conn,
            workspaces: self.workspaces.clone(),
            teams: get_cached_team_options(cx),
        };

        self.editing_connection_id = None;

        open_popup_window(
            PopupWindowOptions::new(if config.editing_connection.is_some() {
                t!("Connection.edit", db_type = db_type.as_str()).to_string()
            } else {
                t!("Connection.new", db_type = db_type.as_str()).to_string()
            })
            .size(700.0, 650.0),
            move |window, cx| cx.new(|cx| ConnectionFormWindow::new(config, window, cx)),
            cx,
        );
    }

    pub(crate) fn show_ssh_form(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let editing_conn = self.editing_connection_id.and_then(|id| {
            self.connections
                .iter()
                .find(|c| c.id == Some(id) && c.connection_type == ConnectionType::SshSftp)
                .cloned()
        });

        let config = SshFormWindowConfig {
            editing_connection: editing_conn,
            workspaces: self.workspaces.clone(),
            teams: get_cached_team_options(cx),
        };

        self.editing_connection_id = None;

        open_popup_window(
            PopupWindowOptions::new(if config.editing_connection.is_some() {
                t!("SSH.edit").to_string()
            } else {
                t!("SSH.new").to_string()
            })
            .size(700.0, 650.0),
            move |window, cx| cx.new(|cx| SshFormWindow::new(config, window, cx)),
            cx,
        );
    }

    pub(crate) fn show_redis_form(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let editing_conn = self.editing_connection_id.and_then(|id| {
            self.connections
                .iter()
                .find(|c| c.id == Some(id) && c.connection_type == ConnectionType::Redis)
                .cloned()
        });

        let config = RedisFormWindowConfig {
            editing_connection: editing_conn,
            workspaces: self.workspaces.clone(),
            teams: get_cached_team_options(cx),
        };

        self.editing_connection_id = None;

        open_popup_window(
            PopupWindowOptions::new(if config.editing_connection.is_some() {
                t!("Connection.edit", db_type = "Redis").to_string()
            } else {
                t!("Connection.new", db_type = "Redis").to_string()
            })
            .size(700.0, 650.0),
            move |window, cx| cx.new(|cx| RedisFormWindow::new(config, window, cx)),
            cx,
        );
    }

    pub(crate) fn show_mongodb_form(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let editing_conn = self.editing_connection_id.and_then(|id| {
            self.connections
                .iter()
                .find(|c| c.id == Some(id) && c.connection_type == ConnectionType::MongoDB)
                .cloned()
        });

        let config = MongoFormWindowConfig {
            editing_connection: editing_conn,
            workspaces: self.workspaces.clone(),
            teams: get_cached_team_options(cx),
        };

        self.editing_connection_id = None;

        open_popup_window(
            PopupWindowOptions::new(if config.editing_connection.is_some() {
                t!("Connection.edit", db_type = "MongoDB").to_string()
            } else {
                t!("Connection.new", db_type = "MongoDB").to_string()
            })
            .size(700.0, 520.0),
            move |window, cx| cx.new(|cx| MongoFormWindow::new(config, window, cx)),
            cx,
        );
    }

    pub(crate) fn show_serial_form(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let editing_conn = self.editing_connection_id.and_then(|id| {
            self.connections
                .iter()
                .find(|c| c.id == Some(id) && c.connection_type == ConnectionType::Serial)
                .cloned()
        });

        let config = SerialFormWindowConfig {
            editing_connection: editing_conn,
            workspaces: self.workspaces.clone(),
            teams: get_cached_team_options(cx),
        };

        self.editing_connection_id = None;

        open_popup_window(
            PopupWindowOptions::new(if config.editing_connection.is_some() {
                t!("Serial.edit").to_string()
            } else {
                t!("Serial.new").to_string()
            })
            .size(700.0, 600.0),
            move |window, cx| cx.new(|cx| SerialFormWindow::new(config, window, cx)),
            cx,
        );
    }

    fn render_toolbar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();

        let workspace_filter_open = self.workspace_filter_open;
        let workspace_filter =
            self.render_workspace_filter_popover(workspace_filter_open, window, cx);

        h_flex()
            .gap_3()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .items_center()
            // ===== 左侧功能区 =====
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    // 新建连接按钮（主要操作）
                    .child(
                        Button::new("new-connect-button")
                            .icon(IconName::Plus)
                            .label(t!("Home.new_connection"))
                            .text_color(cx.theme().primary_foreground)
                            .bg(cx.theme().primary)
                            .with_variant(ButtonVariant::Custom(
                                ButtonCustomVariant::new(cx).hover(cx.theme().primary),
                            ))
                            .tooltip(t!("Home.new_connection"))
                            .dropdown_menu(move |menu, window, _cx| {
                                let mut menu = menu
                                    .item(
                                        PopupMenuItem::new(t!("Workspace.label"))
                                            .icon(
                                                IconName::AppsColor.color().with_size(Size::Medium),
                                            )
                                            .on_click(window.listener_for(
                                                &view,
                                                move |this, _, window, cx| {
                                                    this.show_workspace_form(None, window, cx);
                                                },
                                            )),
                                    )
                                    .separator()
                                    .item(
                                        PopupMenuItem::new("SSH")
                                            .icon(
                                                IconName::TerminalColor
                                                    .color()
                                                    .with_size(Size::Medium),
                                            )
                                            .on_click(window.listener_for(
                                                &view,
                                                move |this, _, window, cx| {
                                                    this.editing_connection_id = None;
                                                    this.show_ssh_form(window, cx);
                                                },
                                            )),
                                    )
                                    .item(
                                        PopupMenuItem::new("Terminal")
                                            .icon(
                                                IconName::Terminal
                                                    .mono()
                                                    .text_color(gpui::rgb(0x8b5cf6))
                                                    .with_size(Size::Medium),
                                            )
                                            .on_click(window.listener_for(
                                                &view,
                                                move |this, _, window, cx| {
                                                    this.add_terminal_tab(window, cx);
                                                },
                                            )),
                                    )
                                    .item(
                                        PopupMenuItem::new("Redis")
                                            .icon(IconName::Redis.color().with_size(Size::Medium))
                                            .on_click(window.listener_for(
                                                &view,
                                                move |this, _, window, cx| {
                                                    this.editing_connection_id = None;
                                                    this.show_redis_form(window, cx);
                                                },
                                            )),
                                    )
                                    .item(
                                        PopupMenuItem::new("MongoDB")
                                            .icon(IconName::MongoDB.color().with_size(Size::Medium))
                                            .on_click(window.listener_for(
                                                &view,
                                                move |this, _, window, cx| {
                                                    this.editing_connection_id = None;
                                                    this.show_mongodb_form(window, cx);
                                                },
                                            )),
                                    )
                                    .item(
                                        PopupMenuItem::new(t!("Serial.new"))
                                            .icon(
                                                IconName::SerialPort
                                                    .color()
                                                    .with_size(Size::Medium),
                                            )
                                            .on_click(window.listener_for(
                                                &view,
                                                move |this, _, window, cx| {
                                                    this.editing_connection_id = None;
                                                    this.show_serial_form(window, cx);
                                                },
                                            )),
                                    )
                                    .separator();

                                for db_type in DatabaseType::all() {
                                    let db_type = *db_type;
                                    let label: SharedString = db_type.as_str().to_string().into();
                                    menu = menu.item(
                                        PopupMenuItem::new(label)
                                            .icon(db_type.as_node_icon().with_size(Size::Medium))
                                            .on_click(window.listener_for(
                                                &view,
                                                move |this, _, window, cx| {
                                                    this.editing_connection_id = None;
                                                    this.show_connection_form(db_type, window, cx);
                                                },
                                            )),
                                    );
                                }

                                menu
                            }),
                    ),
            )
            // ===== 中间弹性空间 =====
            .child(div().flex_1())
            // ===== 右侧操作区 =====
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    // 搜索框
                    .child(
                        Input::new(&self.search_input)
                            .cleanable(true)
                            .w(px(240.0))
                            .bg(cx.theme().muted),
                    )
                    // 刷新按钮
                    .child(
                        Button::new("refresh-button")
                            .icon(IconName::Refresh)
                            .ghost()
                            .tooltip(t!("Home.refresh"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.refresh_local_home_data(cx);
                            })),
                    )
                    // 工作区筛选
                    .child(workspace_filter),
            )
    }

    fn render_workspace_filter_popover(
        &mut self,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let view = cx.entity();
        let view_for_select = view.clone();
        let view_for_clear = view.clone();

        let list = self.ensure_workspace_filter_list(window, cx);

        let workspaces = &self.workspaces;
        let connections = &self.connections;
        let filtered_ids = &self.filtered_workspace_ids;
        list.update(cx, |state, _cx| {
            state
                .delegate_mut()
                .update_items_with_data(workspaces, connections, filtered_ids);
        });

        let is_all_selected = self.filtered_workspace_ids.is_empty()
            || self.filtered_workspace_ids.len()
                == self.workspaces.iter().filter(|w| w.id.is_some()).count();

        Popover::new("workspace-filter-popover")
            .trigger(
                Button::new("workspace-filter")
                    .icon(IconName::Filter)
                    .tooltip(t!("Workspace.filter")),
            )
            .open(open)
            .on_open_change(cx.listener(|this, open, _, cx| {
                this.workspace_filter_open = *open;
                cx.notify();
            }))
            .content(move |_, _, cx| {
                v_flex()
                    .w(px(280.0))
                    .max_h(px(400.0))
                    .gap_2()
                    .p_2()
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .px_1()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child({
                                        let view_select = view_for_select.clone();
                                        Checkbox::new("select-all-ws")
                                            .checked(is_all_selected)
                                            .on_click(move |_, _, cx| {
                                                view_select.update(cx, |this, cx| {
                                                    if this.filtered_workspace_ids.is_empty()
                                                        || this.filtered_workspace_ids.len()
                                                            == this
                                                                .workspaces
                                                                .iter()
                                                                .filter(|w| w.id.is_some())
                                                                .count()
                                                    {
                                                        this.clear_workspace_filter(cx);
                                                    } else {
                                                        this.select_all_workspaces(cx);
                                                    }
                                                });
                                            })
                                    })
                                    .child(div().text_sm().child(
                                        t!("Workspace.select_all").to_string().into_any_element(),
                                    )),
                            )
                            .child({
                                let view_clear = view_for_clear.clone();
                                Button::new("clear-ws-filter")
                                    .ghost()
                                    .small()
                                    .label(t!("Workspace.clear_filter"))
                                    .on_click(move |_, _, cx| {
                                        view_clear.update(cx, |this, cx| {
                                            this.clear_workspace_filter(cx);
                                        });
                                    })
                            }),
                    )
                    .child(div().border_t_1().border_color(cx.theme().border))
                    .child(
                        List::new(&list)
                            .w_full()
                            .max_h(px(320.0))
                            .p(px(8.))
                            .flex_1()
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded(cx.theme().radius),
                    )
            })
            .into_any_element()
    }

    fn ensure_workspace_filter_list(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<ListState<WorkspaceFilterDelegate>> {
        if let Some(ref list) = self.workspace_filter_list {
            return list.clone();
        }

        let parent = cx.entity();
        let list = cx.new(|cx| {
            ListState::new(WorkspaceFilterDelegate::new(parent), window, cx).searchable(true)
        });
        self.workspace_filter_list = Some(list.clone());
        list
    }

    pub(crate) fn toggle_workspace_filter(&mut self, workspace_id: i64, cx: &mut Context<Self>) {
        if self.filtered_workspace_ids.is_empty() {
            for ws in &self.workspaces {
                if let Some(id) = ws.id {
                    self.filtered_workspace_ids.insert(id);
                }
            }
        }

        if self.filtered_workspace_ids.contains(&workspace_id) {
            self.filtered_workspace_ids.remove(&workspace_id);
        } else {
            self.filtered_workspace_ids.insert(workspace_id);
        }
        cx.notify();
    }

    fn select_all_workspaces(&mut self, cx: &mut Context<Self>) {
        self.filtered_workspace_ids.clear();
        for ws in &self.workspaces {
            if let Some(id) = ws.id {
                self.filtered_workspace_ids.insert(id);
            }
        }
        cx.notify();
    }

    fn clear_workspace_filter(&mut self, cx: &mut Context<Self>) {
        self.filtered_workspace_ids.clear();
        cx.notify();
    }

    fn render_sidebar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let filter_types = ConnectionType::all();

        v_flex()
            .w(px(200.0))
            .h_full()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                // 侧边栏过滤选项
                v_flex()
                    .flex_1()
                    .w_full()
                    .p_2()
                    .gap_2()
                    .children(filter_types.into_iter().map(|filter_type| {
                        let is_selected = self.selected_filter == filter_type;
                        let filter_type_clone = filter_type;

                        div()
                            .id(filter_type.label())
                            .flex()
                            .items_center()
                            .gap_3()
                            .w_full()
                            .px_3()
                            .py_2()
                            .cursor_pointer()
                            .rounded_lg()
                            .overflow_hidden()
                            .when(is_selected, |this| {
                                this.bg(cx.theme().list_active)
                                    .border_l_3()
                                    .border_color(cx.theme().list_active_border)
                            })
                            .when(!is_selected, |this| {
                                this.bg(cx.theme().sidebar)
                                    .hover(|style| style.bg(cx.theme().sidebar_accent))
                            })
                            .on_click(cx.listener(move |this: &mut HomePage, _, window, cx| {
                                if filter_type_clone == ConnectionType::ChatDB {
                                    this.add_ai_chat_tab(window, cx);
                                    return;
                                }
                                this.selected_filter = filter_type_clone;
                                cx.notify();
                            }))
                            .child(Icon::new(filter_type.icon()).color().with_size(Size::Large))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().foreground)
                                    .when(is_selected, |this| this.font_weight(FontWeight::MEDIUM))
                                    .child(filter_type.label()),
                            )
                    }))
                    .child(
                        // API 测试入口
                        div()
                            .id("api-tester")
                            .flex()
                            .items_center()
                            .gap_3()
                            .w_full()
                            .px_3()
                            .py_2()
                            .cursor_pointer()
                            .rounded_lg()
                            .overflow_hidden()
                            .bg(cx.theme().sidebar)
                            .hover(|style| style.bg(cx.theme().sidebar_accent))
                            .on_click(cx.listener(move |this: &mut HomePage, _, window, cx| {
                                this.add_api_tab(window, cx);
                            }))
                            .child(Icon::new(IconName::Globe).color().with_size(Size::Large))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().foreground)
                                    .child("API Tester"),
                            )
                    ),
            )
            .child(
                // 底部区域：主题切换、设置和用户头像
                v_flex()
                    .w_full()
                    .p_4()
                    .gap_3()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("open_settings")
                            .icon(IconName::Settings)
                            .label(t!("Common.settings"))
                            .w_full()
                            .justify_start()
                            .on_click(move |_, _window, cx| {
                                crate::settings_window::open_settings_window(cx);
                            }),
                    )
            )
    }

    fn match_connection_type(&self, conn: &StoredConnection) -> bool {
        match self.selected_filter {
            ConnectionType::All => true,
            filter_type => conn.connection_type == filter_type,
        }
    }

    fn match_connection(&self, conn: &StoredConnection, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }

        // 匹配连接名称
        if conn.name.to_lowercase().contains(query) {
            return true;
        }

        // 根据连接类型解析对应参数进行匹配
        match conn.connection_type {
            ConnectionType::Database => {
                if let Ok(params) = conn.to_db_connection() {
                    if params.host.to_lowercase().contains(query) {
                        return true;
                    }
                    if params.port.to_string().contains(query) {
                        return true;
                    }
                    if params.username.to_lowercase().contains(query) {
                        return true;
                    }
                    if params
                        .database
                        .as_ref()
                        .map_or(false, |db| db.to_lowercase().contains(query))
                    {
                        return true;
                    }
                    let conn_str = format!("{}@{}:{}", params.username, params.host, params.port);
                    if conn_str.to_lowercase().contains(query) {
                        return true;
                    }
                }
            }
            ConnectionType::SshSftp => {
                if let Ok(params) = conn.to_ssh_params() {
                    if params.host.to_lowercase().contains(query) {
                        return true;
                    }
                    if params.port.to_string().contains(query) {
                        return true;
                    }
                    if params.username.to_lowercase().contains(query) {
                        return true;
                    }
                    let conn_str = format!("{}@{}:{}", params.username, params.host, params.port);
                    if conn_str.to_lowercase().contains(query) {
                        return true;
                    }
                }
            }
            ConnectionType::Redis => {
                if let Ok(params) = conn.to_redis_params() {
                    if params.host.to_lowercase().contains(query) {
                        return true;
                    }
                    if params.port.to_string().contains(query) {
                        return true;
                    }
                    if params
                        .username
                        .as_ref()
                        .map_or(false, |u| u.to_lowercase().contains(query))
                    {
                        return true;
                    }
                }
            }
            ConnectionType::MongoDB => {
                if let Ok(params) = conn.to_mongodb_params() {
                    if params.host.to_lowercase().contains(query) {
                        return true;
                    }
                    if params.port.map_or(false, |p| p.to_string().contains(query)) {
                        return true;
                    }
                    if params
                        .username
                        .as_ref()
                        .map_or(false, |u| u.to_lowercase().contains(query))
                    {
                        return true;
                    }
                    if params
                        .database
                        .as_ref()
                        .map_or(false, |db| db.to_lowercase().contains(query))
                    {
                        return true;
                    }
                    if params.connection_string.to_lowercase().contains(query) {
                        return true;
                    }
                }
            }
            _ => {}
        }

        false
    }

    fn render_content_area(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let search_query = self.search_query.read(cx).to_lowercase();
        let selected_id = self.selected_connection_id;
        self.render_workspace_view(&search_query, selected_id, cx)
            .into_any_element()
    }

    fn render_workspace_view(
        &self,
        search_query: &str,
        selected_id: Option<i64>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let workspaces_with_connections: Vec<_> = self
            .workspaces
            .iter()
            .filter(|ws| {
                if self.filtered_workspace_ids.is_empty() {
                    return true;
                }
                match ws.id {
                    Some(id) => self.filtered_workspace_ids.contains(&id),
                    None => true,
                }
            })
            .map(|ws| {
                let conn_list: Vec<_> = self
                    .connections
                    .iter()
                    .filter(|conn| conn.workspace_id == ws.id)
                    .filter(|conn| self.match_connection(conn, search_query))
                    .filter(|conn| self.match_connection_type(conn))
                    .cloned()
                    .collect();
                (ws.clone(), conn_list)
            })
            .collect();

        let unassigned_connections: Vec<_> = self
            .connections
            .iter()
            .filter(|conn| conn.workspace_id.is_none())
            .filter(|conn| self.match_connection(conn, search_query))
            .filter(|conn| self.match_connection_type(conn))
            .cloned()
            .collect();

        div()
            .id("home-content")
            .size_full()
            .overflow_y_scroll()
            .p_6()
            .child({
                let mut container = v_flex().gap_8().w_full();

                // 过滤掉空的工作区
                for (workspace, connections) in workspaces_with_connections {
                    if connections.is_empty() {
                        continue;
                    }
                    container = container.child(self.render_workspace_section(
                        workspace,
                        connections,
                        selected_id,
                        cx,
                    ));
                }

                // 如果用户没有设置工作区，直接显示连接列表；否则显示未分配工作区
                if !unassigned_connections.is_empty() {
                    let has_workspaces = self.workspaces.iter().any(|ws| ws.id.is_some());
                    if has_workspaces {
                        container = container.child(self.render_unassigned_section(
                            unassigned_connections,
                            selected_id,
                            cx,
                        ));
                    } else {
                        // 没有工作区时，直接显示连接卡片
                        container = container.child(self.render_connections_grid(
                            unassigned_connections,
                            selected_id,
                            cx,
                        ));
                    }
                }

                container
            })
    }

    fn render_workspace_section(
        &self,
        workspace: Workspace,
        connections: Vec<StoredConnection>,
        selected_id: Option<i64>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let workspace_id = workspace.id;
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .child(
                        Icon::new(IconName::AppsColor)
                            .color()
                            .with_size(Size::Medium),
                    )
                    .child(
                        div()
                            .id(ElementId::Name(SharedString::from(format!(
                                "workspace-name-{}",
                                workspace_id.unwrap_or(0)
                            ))))
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child(workspace.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                t!("Home.connection_count", count = connections.len()).to_string(),
                            ),
                    )
                    .child(div().flex_1()),
            )
            .when(!connections.is_empty(), |this| {
                // 使用 flex 布局实现响应式卡片网格
                let mut container = div().flex().flex_wrap().w_full().gap_3();

                for conn in connections {
                    container = container.child(
                        div()
                            .w(px(320.0)) // 固定宽度，不增长
                            .flex_shrink_0() // 不收缩
                            .child(self.render_connection_card(
                                conn,
                                workspace_id,
                                selected_id,
                                cx,
                            )),
                    );
                }

                this.child(container)
            })
    }

    fn render_connections_grid(
        &self,
        connections: Vec<StoredConnection>,
        selected_id: Option<i64>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut container = div().flex().flex_wrap().w_full().gap_3();

        for conn in connections {
            container = container.child(
                div()
                    .w(px(320.0))
                    .flex_shrink_0()
                    .child(self.render_connection_card(conn, None, selected_id, cx)),
            );
        }
        container
    }

    fn render_unassigned_section(
        &self,
        connections: Vec<StoredConnection>,
        selected_id: Option<i64>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child(
                                t!("Home.unassigned_workspace")
                                    .to_string()
                                    .into_any_element(),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                t!("Home.connection_count", count = connections.len()).to_string(),
                            ),
                    ),
            )
            .child({
                // 使用 flex 布局实现响应式卡片网格
                let mut container = div().flex().flex_wrap().w_full().gap_3();

                for conn in connections {
                    container = container.child(
                        div()
                            .w(px(320.0)) // 固定宽度，不增长
                            .flex_shrink_0() // 不收缩
                            .child(self.render_connection_card(conn, None, selected_id, cx)),
                    );
                }
                container
            })
    }

    fn render_connection_card(
        &self,
        conn: StoredConnection,
        workspace_id: Option<i64>,
        selected_id: Option<i64>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let conn_id = conn.id;
        let clone_conn = conn.clone();
        let sftp_hover_conn = conn.clone();
        let edit_conn = conn.clone();
        let edit_conn_type = conn.connection_type;
        let edit_conn_name = conn.name.clone();
        let delete_conn_id = conn.id;
        let delete_conn_name = conn.name.clone();
        let is_selected = selected_id == conn.id;
        let workspace =
            workspace_id.and_then(|id| self.workspaces.iter().find(|w| w.id == Some(id)).cloned());

        let is_active = conn
            .id
            .map_or(false, |id| cx.global::<ActiveConnections>().is_active(id));

        let can_edit = can_edit_connection(&conn, cx);
        let has_team = conn.team_id.is_some();

        let card = v_flex()
            .justify_center()
            .id(SharedString::from(format!(
                "conn-card-{}",
                conn.id.unwrap_or(0)
            )))
            .w_full()
            .h(px(90.))
            .rounded(px(8.0))
            .bg(cx.theme().background)
            .p_3()
            .border_1()
            .rounded_lg()
            .relative()
            .overflow_hidden()
            .shadow_sm()
            .group("")
            .when(is_selected, |this| {
                this.border_color(cx.theme().list_active_border)
                    .shadow_lg()
                    .border_l_3()
            })
            .when(!is_selected, |this| this.border_color(cx.theme().border))
            .cursor_pointer()
            .hover(|style| {
                style
                    .shadow_lg()
                    .border_color(cx.theme().list_active_border)
            })
            .on_double_click(cx.listener(move |this, _, w, cx| {
                let strategy =
                    build_connection_open_strategy(clone_conn.clone(), workspace.clone());
                strategy.open(this, w, cx);
                cx.notify()
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected_connection_id = conn_id;
                cx.notify();
            }))
            .when(is_active, |this| {
                this.child(
                    div()
                        .absolute()
                        .top(px(6.0))
                        .left(px(6.0))
                        .w(px(10.0))
                        .h(px(10.0))
                        .rounded_full()
                        .bg(cx.theme().success)
                        .shadow_lg(),
                )
            })
            .child(
                // hover时显示的编辑和删除按钮
                h_flex()
                    .absolute()
                    .top_2()
                    .right_2()
                    .gap_1()
                    .group_hover("", |style| style.opacity(1.0))
                    .opacity(0.0)
                    .when(conn.connection_type == ConnectionType::SshSftp, |this| {
                        this.child(
                            Button::new(SharedString::from(format!(
                                "sftp-conn-{}",
                                conn.id.unwrap_or(0)
                            )))
                            .icon(IconName::Folder1.color())
                            .with_size(Size::Small)
                            .primary()
                            .tooltip(t!("Home.open_sftp"))
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.open_sftp_view(sftp_hover_conn.clone(), window, cx);
                                },
                            )),
                        )
                    })
                    .when(can_edit, |this| {
                        this.child(
                            Button::new(SharedString::from(format!(
                                "edit-conn-{}",
                                conn.id.unwrap_or(0)
                            )))
                            .icon(IconName::Edit)
                            .with_size(Size::Small)
                            .primary()
                            .tooltip(t!("Home.edit_connection"))
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    if let Some(conn_id) = edit_conn.id {
                                        let conn_name = edit_conn_name.clone();
                                        match edit_conn_type {
                                            ConnectionType::SshSftp => {
                                                this.editing_connection_id = Some(conn_id);
                                                this.show_ssh_form(window, cx);
                                            }
                                            ConnectionType::Database => {
                                                let db_type = edit_conn
                                                    .to_db_connection()
                                                    .ok()
                                                    .map(|p| p.database_type);
                                                this.confirm_edit_connection(
                                                    conn_id, conn_name, db_type, window, cx,
                                                );
                                            }
                                            ConnectionType::Redis => {
                                                this.editing_connection_id = Some(conn_id);
                                                this.show_redis_form(window, cx);
                                            }
                                            ConnectionType::MongoDB => {
                                                this.editing_connection_id = Some(conn_id);
                                                this.show_mongodb_form(window, cx);
                                            }
                                            ConnectionType::Serial => {
                                                this.editing_connection_id = Some(conn_id);
                                                this.show_serial_form(window, cx);
                                            }
                                            _ => {}
                                        }
                                    }
                                },
                            )),
                        )
                        .child(
                            Button::new(SharedString::from(format!(
                                "delete-conn-{}",
                                conn.id.unwrap_or(0)
                            )))
                            .icon(IconName::Remove)
                            .with_size(Size::Small)
                            .danger()
                            .tooltip(t!("Home.delete_connection"))
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    if let Some(conn_id) = delete_conn_id {
                                        let conn_name = delete_conn_name.clone();
                                        this.confirm_delete_connection(
                                            conn_id, conn_name, window, cx,
                                        );
                                    }
                                },
                            )),
                        )
                    }),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .w_full()
                    .child(
                        div()
                            .h(px(48.0))
                            .rounded(px(8.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(match conn.connection_type {
                                ConnectionType::Database => {
                                    let icon = conn
                                        .to_db_connection()
                                        .map(|c| c.database_type.as_icon())
                                        .unwrap_or_else(|_| IconName::Database.color());
                                    icon.with_size(px(40.0)).text_color(gpui::white())
                                }
                                ConnectionType::SshSftp => IconName::TerminalColor
                                    .color()
                                    .with_size(px(40.0))
                                    .text_color(gpui::rgb(0x8b5cf6)),
                                ConnectionType::Redis => IconName::Redis
                                    .color()
                                    .with_size(px(40.0))
                                    .text_color(gpui::white()),
                                ConnectionType::MongoDB => IconName::MongoDB
                                    .color()
                                    .with_size(px(40.0))
                                    .text_color(gpui::white()),
                                ConnectionType::Serial => IconName::SerialPort
                                    .color()
                                    .with_size(px(40.0))
                                    .text_color(gpui::white()),
                                _ => IconName::Server
                                    .color()
                                    .with_size(px(40.0))
                                    .text_color(gpui::white()),
                            }),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .overflow_hidden()
                            .child({
                                let name_tooltip: SharedString = conn.name.clone().into();
                                h_flex()
                                    .gap_1()
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "conn-name-{}",
                                                conn.id.unwrap_or(0)
                                            )))
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().foreground)
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .whitespace_nowrap()
                                            .flex_shrink()
                                            .min_w_0()
                                            .tooltip(move |window, cx| {
                                                Tooltip::new(name_tooltip.clone()).build(window, cx)
                                            })
                                            .child(conn.name.clone()),
                                    )
                                    .when(has_team, |this| {
                                        this.child(
                                            div()
                                                .flex_shrink_0()
                                                .px_1()
                                                .rounded(px(3.0))
                                                .bg(cx.theme().accent.opacity(0.15))
                                                .text_color(cx.theme().accent)
                                                .text_xs()
                                                .child(t!("Home.team_badge").to_string()),
                                        )
                                    })
                            })
                            .when(conn.connection_type == ConnectionType::Database, |this| {
                                if let Ok(params) = conn.to_db_connection() {
                                    let conn_info = if params.database_type == DatabaseType::SQLite
                                    {
                                        params.host.clone()
                                    } else {
                                        let database = match params.database {
                                            Some(database) => format!("/{}", database),
                                            None => "".to_string(),
                                        };
                                        format!(
                                            "{}@{}:{}{}",
                                            params.username, params.host, params.port, database
                                        )
                                    };
                                    let tooltip_text: SharedString = conn_info.clone().into();
                                    this.child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "conn-info-{}",
                                                conn.id.unwrap_or(0)
                                            )))
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .whitespace_nowrap()
                                            .max_w_full()
                                            .tooltip(move |window, cx| {
                                                Tooltip::new(tooltip_text.clone()).build(window, cx)
                                            })
                                            .child(conn_info),
                                    )
                                } else {
                                    this
                                }
                            })
                            .when(conn.connection_type == ConnectionType::SshSftp, |this| {
                                if let Ok(params) = conn.to_ssh_params() {
                                    let conn_info = format!(
                                        "{}@{}:{}",
                                        params.username, params.host, params.port
                                    );
                                    let tooltip_text: SharedString = conn_info.clone().into();
                                    this.child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "conn-info-{}",
                                                conn.id.unwrap_or(0)
                                            )))
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .whitespace_nowrap()
                                            .max_w_full()
                                            .tooltip(move |window, cx| {
                                                Tooltip::new(tooltip_text.clone()).build(window, cx)
                                            })
                                            .child(conn_info),
                                    )
                                } else {
                                    this
                                }
                            })
                            .when(conn.connection_type == ConnectionType::Redis, |this| {
                                if let Ok(params) = conn.to_redis_params() {
                                    let conn_info = match params.mode {
                                        RedisMode::Standalone => {
                                            format!(
                                                "{}:{}/{}",
                                                params.host, params.port, params.db_index
                                            )
                                        }
                                        RedisMode::Sentinel => {
                                            let (master_name, sentinel_count) = params
                                                .sentinel
                                                .as_ref()
                                                .map(|sentinel| {
                                                    (
                                                        sentinel.master_name.as_str(),
                                                        sentinel.sentinels.len(),
                                                    )
                                                })
                                                .unwrap_or(("sentinel", 0));
                                            format!("{} (sentinel:{})", master_name, sentinel_count)
                                        }
                                        RedisMode::Cluster => {
                                            let node_count = params
                                                .cluster
                                                .as_ref()
                                                .map(|cluster| cluster.nodes.len())
                                                .unwrap_or(0);
                                            format!("cluster ({} nodes)", node_count)
                                        }
                                    };
                                    let tooltip_text: SharedString = conn_info.clone().into();
                                    this.child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "conn-info-{}",
                                                conn.id.unwrap_or(0)
                                            )))
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .whitespace_nowrap()
                                            .max_w_full()
                                            .tooltip(move |window, cx| {
                                                Tooltip::new(tooltip_text.clone()).build(window, cx)
                                            })
                                            .child(conn_info),
                                    )
                                } else {
                                    this
                                }
                            })
                            .when(conn.connection_type == ConnectionType::MongoDB, |this| {
                                if let Ok(params) = conn.to_mongodb_params() {
                                    let conn_info = if !params.host.is_empty() {
                                        if let Some(port) = params.port {
                                            format!("{}:{}", params.host, port)
                                        } else {
                                            params.host
                                        }
                                    } else if !params.connection_string.is_empty() {
                                        params.connection_string
                                    } else {
                                        "MongoDB".to_string()
                                    };
                                    let tooltip_text: SharedString = conn_info.clone().into();
                                    this.child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "conn-info-{}",
                                                conn.id.unwrap_or(0)
                                            )))
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .whitespace_nowrap()
                                            .max_w_full()
                                            .tooltip(move |window, cx| {
                                                Tooltip::new(tooltip_text.clone()).build(window, cx)
                                            })
                                            .child(conn_info),
                                    )
                                } else {
                                    this
                                }
                            })
                            .when(conn.connection_type == ConnectionType::Serial, |this| {
                                if let Ok(params) = conn.to_serial_params() {
                                    // 格式：/dev/ttyUSB0 (115200, 8N1)
                                    let parity_char = match params.parity {
                                        one_core::storage::models::SerialParity::None => 'N',
                                        one_core::storage::models::SerialParity::Odd => 'O',
                                        one_core::storage::models::SerialParity::Even => 'E',
                                    };
                                    let conn_info = format!(
                                        "{} ({}, {}{}{})",
                                        params.port_name,
                                        params.baud_rate,
                                        params.data_bits,
                                        parity_char,
                                        params.stop_bits,
                                    );
                                    let tooltip_text: SharedString = conn_info.clone().into();
                                    this.child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "conn-info-{}",
                                                conn.id.unwrap_or(0)
                                            )))
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .whitespace_nowrap()
                                            .max_w_full()
                                            .tooltip(move |window, cx| {
                                                Tooltip::new(tooltip_text.clone()).build(window, cx)
                                            })
                                            .child(conn_info),
                                    )
                                } else {
                                    this
                                }
                            }),
                    ),
            );

        card.into_any_element()
    }
}

impl Focusable for HomePage {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<TabContentEvent> for HomePage {}

impl TabContent for HomePage {
    fn content_key(&self) -> &'static str {
        "Home"
    }

    fn title(&self, _cx: &App) -> SharedString {
        SharedString::from(t!("Home.title"))
    }

    fn icon(&self, _cx: &App) -> Option<Icon> {
        Some(IconName::Home.color())
    }

    fn closeable(&self, _cx: &App) -> bool {
        false
    }

    fn width_size(&self, _cx: &App) -> Option<Size> {
        Some(Size::Small)
    }
}

impl Render for HomePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().track_focus(&self.focus_handle).child(
            h_flex()
                .size_full()
                .child(self.render_sidebar(window, cx))
                .child(
                    v_flex()
                        .flex_1()
                        .h_full()
                        .bg(cx.theme().background)
                        .child(self.render_toolbar(window, cx))
                        .child(
                            div()
                                .flex_1()
                                .w_full()
                                .overflow_hidden()
                                .bg(cx.theme().muted)
                                .child(self.render_content_area(cx)),
                        ),
                ),
        )
    }
}

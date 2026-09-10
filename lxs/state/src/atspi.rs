//! Native AT-SPI tree walking and element interaction, following cua-driver.
//!
//! Connects to the AT-SPI registry over D-Bus via the `atspi` crate (zbus).
//! Walks applications by PID, resolves element bounds/actions through the
//! Component and Action interfaces, and exposes richer state (value, checked,
//! enabled, selected, description) like cua-driver's native walker. Per-call
//! timeouts keep large or wedged apps from blocking the caller.

use std::time::Duration;

use atspi::connection::AccessibilityConnection;
use atspi::connection::P2P;
use atspi::proxy::accessible::AccessibleProxy;
use atspi::proxy::proxy_ext::ProxyExt;
use atspi::CoordType;
use atspi::ScrollType;
use atspi::State;
use lxs_core::{A11yElement, AccessibilityTree, Bounds, LxsError};

const CALL_TIMEOUT: Duration = Duration::from_secs(10);
const OP_TIMEOUT: Duration = Duration::from_secs(25);

static SHARED_CONNECTION: tokio::sync::OnceCell<AccessibilityConnection> =
    tokio::sync::OnceCell::const_new();

async fn shared_connection() -> Result<&'static AccessibilityConnection, LxsError> {
    SHARED_CONNECTION
        .get_or_try_init(|| async {
            AccessibilityConnection::new()
                .await
                .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))
        })
        .await
}

async fn call<T>(fut: impl std::future::Future<Output = T>) -> Option<T> {
    tokio::time::timeout(CALL_TIMEOUT, fut).await.ok()
}

async fn op<T>(fut: impl std::future::Future<Output = Result<T, LxsError>>) -> Result<T, LxsError> {
    tokio::time::timeout(OP_TIMEOUT, fut)
        .await
        .unwrap_or_else(|_| Err(LxsError::NotImplemented))
}

pub struct Element {
    pub index: usize,
    pub role: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    pub checked: Option<bool>,
    pub enabled: Option<bool>,
    pub selected: Option<bool>,
    pub bounds: Option<Bounds>,
    pub actions: Vec<String>,
}

pub async fn walk_tree(pid: u32) -> Result<Vec<Element>, LxsError> {
    op(walk_tree_inner(pid)).await
}

async fn walk_tree_inner(pid: u32) -> Result<Vec<Element>, LxsError> {
    let conn = shared_connection().await?;

    let root = conn
        .root_accessible_on_registry()
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?;

    for attempt in 0..4 {
        if let Some(app) = find_app(conn, &root, pid).await {
            return Ok(walk(conn, &app).await);
        }
        if attempt < 3 {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
    Err(LxsError::DisplayNotFound(format!("pid {}", pid)))
}

async fn find_app(
    conn: &'static AccessibilityConnection,
    root: &AccessibleProxy<'static>,
    pid: u32,
) -> Option<AccessibleProxy<'static>> {
    let dbus = atspi::zbus::fdo::DBusProxy::new(conn.connection())
        .await
        .ok()?;

    let children = root.get_children().await.ok()?;
    for child_ref in children {
        if pid_of(&dbus, &child_ref).await == Some(pid) {
            return conn.object_as_accessible(&child_ref).await.ok();
        }
    }
    None
}

async fn pid_of(
    dbus: &atspi::zbus::fdo::DBusProxy<'_>,
    oref: &atspi::ObjectRefOwned,
) -> Option<u32> {
    let name = oref.name_as_str()?;
    let bus = atspi::zbus::names::BusName::try_from(name.to_owned()).ok()?;
    dbus.get_connection_unix_process_id(bus).await.ok()
}

pub(crate) async fn walk(
    conn: &'static AccessibilityConnection,
    root: &AccessibleProxy<'static>,
) -> Vec<Element> {
    let mut elements = Vec::new();
    let mut stack: Vec<AccessibleProxy<'static>> = vec![root.clone()];

    while let Some(node) = stack.pop() {
        let role = node
            .get_role()
            .await
            .map(|r| r.to_string())
            .unwrap_or_default();
        let name = node.name().await.ok().filter(|s| !s.is_empty());
        let description = node.description().await.ok().filter(|s| !s.is_empty());

        let (checked, enabled, selected) = if let Some(Ok(states)) = call(node.get_state()).await {
            (
                Some(states.contains(State::Checked)),
                Some(states.contains(State::Enabled)),
                Some(states.contains(State::Selected)),
            )
        } else {
            (None, None, None)
        };

        let mut bounds = None;
        let mut actions = Vec::new();
        let mut value = None;

        if let Some(Ok(proxies)) = call(node.proxies()).await {
            if let Some(Ok(component)) = call(proxies.component()).await {
                if let Some(Ok((x, y, w, h))) = call(component.get_extents(CoordType::Screen)).await
                {
                    bounds = Some(Bounds {
                        x,
                        y,
                        w: w as u32,
                        h: h as u32,
                    });
                }
            }
            if let Some(Ok(action)) = call(proxies.action()).await {
                if let Some(Ok(n)) = call(action.n_actions()).await {
                    for i in 0..n {
                        if let Some(Ok(name)) = call(action.get_name(i)).await {
                            if !name.trim().is_empty() {
                                actions.push(name);
                            }
                        }
                    }
                }
            }
            if let Some(Ok(value_proxy)) = call(proxies.value()).await {
                if let Some(Ok(v)) = call(value_proxy.current_value()).await {
                    value = Some(format!("{}", v));
                }
            }
            if value.is_none() {
                if let Some(Ok(text_proxy)) = call(proxies.text()).await {
                    if let Some(Ok(len)) = call(text_proxy.character_count()).await {
                        if len > 0 {
                            if let Some(Ok(t)) = call(text_proxy.get_text(0, len)).await {
                                value = Some(t);
                            }
                        }
                    }
                }
            }
        }

        elements.push(Element {
            index: elements.len(),
            role,
            name,
            description,
            value,
            checked,
            enabled,
            selected,
            bounds,
            actions,
        });

        if let Some(Ok(children)) = call(node.get_children()).await {
            for child_ref in children.into_iter().rev() {
                if let Ok(child) = conn.object_as_accessible(&child_ref).await {
                    stack.push(child);
                }
            }
        }
    }

    elements
}

pub fn accessibility_tree(elements: &[Element]) -> AccessibilityTree {
    AccessibilityTree {
        elements: elements
            .iter()
            .map(|e| A11yElement {
                index: e.index,
                role: e.role.clone(),
                name: e.name.clone(),
                description: e.description.clone(),
                value: e.value.clone(),
                checked: e.checked,
                enabled: e.enabled,
                selected: e.selected,
                actions: e.actions.clone(),
            })
            .collect(),
    }
}

pub async fn perform_action(pid: u32, index: usize, action: &str) -> Result<(), LxsError> {
    op(perform_action_inner(pid, index, action)).await
}

async fn perform_action_inner(pid: u32, index: usize, action: &str) -> Result<(), LxsError> {
    let conn = shared_connection().await?;
    let root = conn
        .root_accessible_on_registry()
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?;
    let app = find_app_with_retry(conn, &root, pid).await?;
    let node = walk_to_index(conn, &app, index)
        .await?
        .ok_or(LxsError::NotImplemented)?;

    let proxies = node.proxies().await.map_err(|_| LxsError::NotImplemented)?;
    let action_proxy = proxies
        .action()
        .await
        .map_err(|_| LxsError::NotImplemented)?;

    let n = action_proxy.n_actions().await.unwrap_or(0);
    for i in 0..n {
        if let Ok(name) = action_proxy.get_name(i).await {
            if name.trim() == action.trim() {
                action_proxy
                    .do_action(i)
                    .await
                    .map_err(|_| LxsError::NotImplemented)?;
                return Ok(());
            }
        }
    }
    Err(LxsError::NotImplemented)
}

pub async fn focus_element(pid: u32, index: usize) -> Result<bool, LxsError> {
    op(focus_element_inner(pid, index)).await
}

async fn focus_element_inner(pid: u32, index: usize) -> Result<bool, LxsError> {
    let conn = shared_connection().await?;
    let root = conn
        .root_accessible_on_registry()
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?;
    let app = find_app_with_retry(conn, &root, pid).await?;
    let node = walk_to_index(conn, &app, index)
        .await?
        .ok_or(LxsError::NotImplemented)?;

    let proxies = node.proxies().await.map_err(|_| LxsError::NotImplemented)?;
    let component = proxies
        .component()
        .await
        .map_err(|_| LxsError::NotImplemented)?;
    component
        .grab_focus()
        .await
        .map_err(|_| LxsError::NotImplemented)
}

pub async fn scroll_element(
    pid: u32,
    index: usize,
    direction: &str,
    amount: u32,
) -> Result<(), LxsError> {
    op(scroll_element_inner(pid, index, direction, amount)).await
}

async fn scroll_element_inner(
    pid: u32,
    index: usize,
    direction: &str,
    _amount: u32,
) -> Result<(), LxsError> {
    let scroll_type = match direction {
        "up" => ScrollType::TopEdge,
        "down" => ScrollType::BottomEdge,
        "left" => ScrollType::LeftEdge,
        "right" => ScrollType::RightEdge,
        _ => {
            return Err(LxsError::InvalidArgument(format!(
                "unknown direction: {}",
                direction
            )))
        }
    };

    let conn = shared_connection().await?;
    let root = conn
        .root_accessible_on_registry()
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?;
    let app = find_app_with_retry(conn, &root, pid).await?;
    let node = walk_to_index(conn, &app, index)
        .await?
        .ok_or(LxsError::NotImplemented)?;

    let proxies = node.proxies().await.map_err(|_| LxsError::NotImplemented)?;
    let component = proxies
        .component()
        .await
        .map_err(|_| LxsError::NotImplemented)?;
    component
        .scroll_to(scroll_type)
        .await
        .map_err(|_| LxsError::NotImplemented)?;
    Ok(())
}

pub async fn set_value(pid: u32, index: usize, value: &str) -> Result<(), LxsError> {
    op(set_value_inner(pid, index, value)).await
}

async fn set_value_inner(pid: u32, index: usize, value: &str) -> Result<(), LxsError> {
    let conn = shared_connection().await?;
    let root = conn
        .root_accessible_on_registry()
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?;
    let app = find_app_with_retry(conn, &root, pid).await?;
    let node = walk_to_index(conn, &app, index)
        .await?
        .ok_or(LxsError::NotImplemented)?;

    let proxies = node.proxies().await.map_err(|_| LxsError::NotImplemented)?;

    if let Ok(editable) = proxies.editable_text().await {
        if editable
            .set_text_contents(value)
            .await
            .map_err(|_| LxsError::NotImplemented)?
        {
            return Ok(());
        }
    }

    let value_proxy = proxies
        .value()
        .await
        .map_err(|_| LxsError::NotImplemented)?;
    let parsed = value.parse::<f64>().map_err(|_| {
        LxsError::InvalidArgument("value must be numeric for Value interface".into())
    })?;
    value_proxy
        .set_current_value(parsed)
        .await
        .map_err(|_| LxsError::NotImplemented)
}

pub async fn type_into_editable(pid: u32, index: usize, text: &str) -> Result<(), LxsError> {
    op(type_into_editable_inner(pid, index, text)).await
}

async fn type_into_editable_inner(pid: u32, index: usize, text: &str) -> Result<(), LxsError> {
    let conn = shared_connection().await?;
    let root = conn
        .root_accessible_on_registry()
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?;
    let app = find_app_with_retry(conn, &root, pid).await?;
    let node = walk_to_index(conn, &app, index)
        .await?
        .ok_or(LxsError::NotImplemented)?;

    let proxies = node.proxies().await.map_err(|_| LxsError::NotImplemented)?;
    let editable = proxies
        .editable_text()
        .await
        .map_err(|_| LxsError::NotImplemented)?;
    editable
        .set_text_contents(text)
        .await
        .map_err(|_| LxsError::NotImplemented)?;
    Ok(())
}

pub async fn find_element(pid: u32, query: &str) -> Result<Option<Element>, LxsError> {
    let elements = walk_tree(pid).await?;
    let lower = query.to_lowercase();
    Ok(elements.into_iter().find(|e| {
        e.role.to_lowercase().contains(&lower)
            || e.name
                .as_ref()
                .map(|s| s.to_lowercase().contains(&lower))
                .unwrap_or(false)
            || e.description
                .as_ref()
                .map(|s| s.to_lowercase().contains(&lower))
                .unwrap_or(false)
            || e.value
                .as_ref()
                .map(|s| s.to_lowercase().contains(&lower))
                .unwrap_or(false)
    }))
}

async fn find_app_with_retry(
    conn: &'static AccessibilityConnection,
    root: &AccessibleProxy<'static>,
    pid: u32,
) -> Result<AccessibleProxy<'static>, LxsError> {
    for attempt in 0..4 {
        if let Some(app) = find_app(conn, root, pid).await {
            return Ok(app);
        }
        if attempt < 3 {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
    Err(LxsError::DisplayNotFound(format!("pid {}", pid)))
}

async fn walk_to_index(
    conn: &'static AccessibilityConnection,
    root: &AccessibleProxy<'static>,
    target_index: usize,
) -> Result<Option<AccessibleProxy<'static>>, LxsError> {
    let mut stack = vec![root.clone()];
    let mut count = 0;

    while let Some(node) = stack.pop() {
        if count == target_index {
            return Ok(Some(node));
        }
        count += 1;

        if let Some(Ok(children)) = call(node.get_children()).await {
            for child_ref in children.into_iter().rev() {
                if let Ok(child) = conn.object_as_accessible(&child_ref).await {
                    stack.push(child);
                }
            }
        }
    }

    Ok(None)
}

//! Native AT-SPI tree walking, matching the approach used by cua-driver.
//!
//! Like cua-driver we connect directly to the AT-SPI registry over D-Bus,
//! walk applications by PID, and resolve element bounds/actions through the
//! Component and Action interfaces. Tree walks retry lazily-registered
//! bridges, and per-call timeouts keep large or wedged apps from blocking the
//! caller. The public functions are async so callers can await them on their
//! own Tokio runtime instead of spawning a nested one.

use std::time::Duration;

use atspi::connection::AccessibilityConnection;
use atspi::connection::P2P;
use atspi::proxy::accessible::AccessibleProxy;
use atspi::proxy::proxy_ext::ProxyExt;
use atspi::CoordType;
use lxs_core::{A11yElement, AccessibilityTree, Bounds, LxsError};

const CALL_TIMEOUT: Duration = Duration::from_secs(10);

pub struct Element {
    pub index: usize,
    pub role: String,
    pub name: Option<String>,
    pub bounds: Option<Bounds>,
    pub actions: Vec<String>,
}

async fn call<T>(fut: impl std::future::Future<Output = T>) -> Option<T> {
    tokio::time::timeout(CALL_TIMEOUT, fut).await.ok()
}

pub async fn walk_tree(pid: u32) -> Result<Vec<Element>, LxsError> {
    let conn = AccessibilityConnection::new()
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?;

    let root = conn
        .root_accessible_on_registry()
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?;

    // Retry: AT-SPI bridges register lazily after an app maps its first window.
    let mut last_error = None;
    for attempt in 0..4 {
        if let Some(app) = find_app(&conn, &root, pid).await {
            return Ok(walk(&conn, &app).await);
        }
        last_error = Some(LxsError::DisplayNotFound(format!("pid {}", pid)));
        if attempt < 3 {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
    Err(last_error.unwrap())
}

async fn find_app<'a>(
    conn: &'a AccessibilityConnection,
    root: &'a AccessibleProxy<'a>,
    pid: u32,
) -> Option<AccessibleProxy<'a>> {
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

pub(crate) async fn walk<'a>(
    conn: &'a AccessibilityConnection,
    root: &'a AccessibleProxy<'a>,
) -> Vec<Element> {
    let mut elements = Vec::new();
    let mut stack: Vec<(AccessibleProxy<'a>, usize)> = vec![(root.clone(), 0)];

    while let Some((node, _depth)) = stack.pop() {
        let role = node
            .get_role()
            .await
            .map(|r| r.to_string())
            .unwrap_or_default();
        let name = node.name().await.ok();

        let mut bounds = None;
        let mut actions = Vec::new();

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
        }

        elements.push(Element {
            index: elements.len(),
            role,
            name,
            bounds,
            actions,
        });

        if let Some(Ok(children)) = call(node.get_children()).await {
            for child_ref in children.into_iter().rev() {
                if let Ok(child) = conn.object_as_accessible(&child_ref).await {
                    stack.push((child, _depth + 1));
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
                actions: e.actions.clone(),
            })
            .collect(),
    }
}

pub async fn perform_action(pid: u32, index: usize, action: &str) -> Result<(), LxsError> {
    let conn = AccessibilityConnection::new()
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?;

    let root = conn
        .root_accessible_on_registry()
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?;

    let mut app = None;
    for attempt in 0..4 {
        if let Some(found) = find_app(&conn, &root, pid).await {
            app = Some(found);
            break;
        }
        if attempt < 3 {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
    let app = app.ok_or_else(|| LxsError::DisplayNotFound(format!("pid {}", pid)))?;

    let node = walk_to_index(&conn, &app, index)
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

async fn walk_to_index<'a>(
    conn: &'a AccessibilityConnection,
    root: &'a AccessibleProxy<'a>,
    target_index: usize,
) -> Result<Option<AccessibleProxy<'a>>, LxsError> {
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

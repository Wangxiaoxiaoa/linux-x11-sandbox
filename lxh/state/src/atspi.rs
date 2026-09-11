use atspi::connection::AccessibilityConnection;
use atspi::connection::P2P;
use atspi::proxy::accessible::AccessibleProxy;
use atspi::proxy::proxy_ext::ProxyExt;
use atspi::CoordType;
use lxh_core::{A11yElement, AccessibilityTree, Bounds, LxhError};

#[derive(Clone)]
pub struct Element {
    pub index: usize,
    pub role: String,
    pub name: Option<String>,
    pub frame: Option<Bounds>,
    pub actions: Vec<String>,
    pub parent_index: Option<usize>,
    pub depth: usize,
}

pub async fn walk_tree(pid: u32) -> Result<Vec<Element>, LxhError> {
    let conn = AccessibilityConnection::new()
        .await
        .map_err(|e| LxhError::ProcessSpawnFailed(e.to_string()))?;

    let root = conn
        .root_accessible_on_registry()
        .await
        .map_err(|e| LxhError::ProcessSpawnFailed(e.to_string()))?;

    let dbus = atspi::zbus::fdo::DBusProxy::new(conn.connection())
        .await
        .map_err(|e| LxhError::ProcessSpawnFailed(e.to_string()))?;

    let children = root
        .get_children()
        .await
        .map_err(|e| LxhError::ProcessSpawnFailed(e.to_string()))?;

    let mut app = None;
    for child_ref in children {
        if pid_of(&dbus, &child_ref).await == Some(pid) {
            app = Some(
                conn.object_as_accessible(&child_ref)
                    .await
                    .map_err(|e| LxhError::ProcessSpawnFailed(e.to_string()))?,
            );
            break;
        }
    }

    let app = app.ok_or_else(|| LxhError::DisplayNotFound(format!("pid {}", pid)))?;
    Ok(walk(&conn, &app).await)
}

async fn pid_of(
    dbus: &atspi::zbus::fdo::DBusProxy<'_>,
    oref: &atspi::ObjectRefOwned,
) -> Option<u32> {
    let name = oref.name_as_str()?;
    let bus = atspi::zbus::names::BusName::try_from(name.to_owned()).ok()?;
    dbus.get_connection_unix_process_id(bus).await.ok()
}

async fn walk<'a>(
    conn: &'a AccessibilityConnection,
    root: &'a AccessibleProxy<'a>,
) -> Vec<Element> {
    let mut elements = Vec::new();
    let mut stack: Vec<(AccessibleProxy<'a>, Option<usize>, usize)> = vec![(root.clone(), None, 0)];

    while let Some((node, parent_index, depth)) = stack.pop() {
        let index = elements.len();
        let role = node
            .get_role()
            .await
            .map(|r| r.to_string())
            .unwrap_or_default();
        let name = node.name().await.ok();

        let mut frame = None;
        let mut actions = Vec::new();

        if let Ok(proxies) = node.proxies().await {
            if let Ok(component) = proxies.component().await {
                if let Ok((x, y, w, h)) = component.get_extents(CoordType::Screen).await {
                    frame = Some(Bounds {
                        x,
                        y,
                        w: w as u32,
                        h: h as u32,
                    });
                }
            }
            if let Ok(action) = proxies.action().await {
                if let Ok(n) = action.n_actions().await {
                    for i in 0..n {
                        if let Ok(name) = action.get_name(i).await {
                            if !name.trim().is_empty() {
                                actions.push(name);
                            }
                        }
                    }
                }
            }
        }

        elements.push(Element {
            index,
            role,
            name,
            frame,
            actions,
            parent_index,
            depth,
        });

        if let Ok(children) = node.get_children().await {
            for child_ref in children.into_iter().rev() {
                if let Ok(child) = conn.object_as_accessible(&child_ref).await {
                    stack.push((child, Some(index), depth + 1));
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
                frame: e.frame.clone(),
                actions: e.actions.clone(),
                parent_index: e.parent_index,
                depth: e.depth,
            })
            .collect(),
    }
}

pub async fn set_value(pid: u32, index: usize, value: &str) -> Result<(), LxhError> {
    let conn = AccessibilityConnection::new()
        .await
        .map_err(|e| LxhError::ProcessSpawnFailed(e.to_string()))?;

    let root = conn
        .root_accessible_on_registry()
        .await
        .map_err(|e| LxhError::ProcessSpawnFailed(e.to_string()))?;

    let dbus = atspi::zbus::fdo::DBusProxy::new(conn.connection())
        .await
        .map_err(|e| LxhError::ProcessSpawnFailed(e.to_string()))?;

    let children = root
        .get_children()
        .await
        .map_err(|e| LxhError::ProcessSpawnFailed(e.to_string()))?;

    let mut app = None;
    for child_ref in children {
        if pid_of(&dbus, &child_ref).await == Some(pid) {
            app = Some(
                conn.object_as_accessible(&child_ref)
                    .await
                    .map_err(|e| LxhError::ProcessSpawnFailed(e.to_string()))?,
            );
            break;
        }
    }

    let app = app.ok_or_else(|| LxhError::DisplayNotFound(format!("pid {}", pid)))?;

    let node = walk_to_index(&conn, &app, index)
        .await?
        .ok_or(LxhError::NotSupported)?;

    let proxies = node.proxies().await.map_err(|_| LxhError::NotSupported)?;
    let editable = proxies
        .editable_text()
        .await
        .map_err(|_| LxhError::NotSupported)?;

    editable
        .set_text_contents(value)
        .await
        .map_err(|_| LxhError::NotSupported)?;

    Ok(())
}

async fn walk_to_index<'a>(
    conn: &'a AccessibilityConnection,
    root: &'a AccessibleProxy<'a>,
    target_index: usize,
) -> Result<Option<AccessibleProxy<'a>>, LxhError> {
    let mut stack = vec![root.clone()];
    let mut count = 0;

    while let Some(node) = stack.pop() {
        if count == target_index {
            return Ok(Some(node));
        }
        count += 1;

        if let Ok(children) = node.get_children().await {
            for child_ref in children.into_iter().rev() {
                if let Ok(child) = conn.object_as_accessible(&child_ref).await {
                    stack.push(child);
                }
            }
        }
    }

    Ok(None)
}

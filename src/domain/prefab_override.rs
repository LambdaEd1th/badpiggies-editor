//! Shared parser/serializer for Unity ObjectDeserializer override text.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverrideNode {
    pub node_type: String,
    pub name: String,
    pub value: Option<String>,
    pub children: Vec<OverrideNode>,
}

impl OverrideNode {
    pub fn child(&self, node_type: &str, name: &str) -> Option<&OverrideNode> {
        self.children
            .iter()
            .find(|node| node.node_type == node_type && node.name == name)
    }

    pub fn child_mut(&mut self, node_type: &str, name: &str) -> Option<&mut OverrideNode> {
        self.children
            .iter_mut()
            .find(|node| node.node_type == node_type && node.name == name)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn children_of_type<'a>(
        &'a self,
        node_type: &'a str,
    ) -> impl Iterator<Item = &'a OverrideNode> + 'a {
        self.children
            .iter()
            .filter(move |node| node.node_type == node_type)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn component(&self, suffix: &str) -> Option<&OverrideNode> {
        self.children.iter().find(|node| {
            node.node_type == "Component"
                && (node.name == suffix
                    || node
                        .name
                        .rsplit('.')
                        .next()
                        .is_some_and(|name| name == suffix))
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn value_as_f32(&self) -> Option<f32> {
        self.value.as_deref()?.parse().ok()
    }

    pub fn value_as_i32(&self) -> Option<i32> {
        self.value.as_deref()?.parse().ok()
    }

    #[allow(dead_code)]
    pub fn value_as_bool(&self) -> Option<bool> {
        let value = self.value.as_deref()?;
        if value.eq_ignore_ascii_case("true") {
            Some(true)
        } else if value.eq_ignore_ascii_case("false") {
            Some(false)
        } else {
            None
        }
    }

    pub fn find_descendant<F>(&self, predicate: &F) -> Option<&OverrideNode>
    where
        F: Fn(&OverrideNode) -> bool,
    {
        if predicate(self) {
            return Some(self);
        }

        self.children
            .iter()
            .find_map(|child| child.find_descendant(predicate))
    }

    pub fn find_descendant_mut<F>(&mut self, predicate: &F) -> Option<&mut OverrideNode>
    where
        F: Fn(&OverrideNode) -> bool,
    {
        if predicate(self) {
            return Some(self);
        }

        for child in &mut self.children {
            if let Some(found) = child.find_descendant_mut(predicate) {
                return Some(found);
            }
        }

        None
    }
}

pub fn parse_override_text(raw: &str) -> Vec<OverrideNode> {
    let lines: Vec<&str> = raw.lines().collect();
    parse_override_range(&lines, 0, lines.len(), 0)
}

fn parse_override_range(
    lines: &[&str],
    start: usize,
    end: usize,
    base_depth: usize,
) -> Vec<OverrideNode> {
    let mut result = Vec::new();
    let mut index = start;

    while index < end {
        let line = lines[index].trim_end_matches('\r');
        let depth = indentation(line);
        let trimmed = line.trim();
        if trimmed.is_empty() || depth < base_depth {
            index += 1;
            continue;
        }
        if depth > base_depth {
            index += 1;
            continue;
        }

        let (node_type, name, value) = parse_override_line(trimmed);

        let child_start = index + 1;
        let mut child_end = child_start;
        while child_end < end {
            let child_line = lines[child_end].trim_end_matches('\r');
            let child_depth = indentation(child_line);
            if child_line.trim().is_empty() {
                child_end += 1;
                continue;
            }
            if child_depth <= depth {
                break;
            }
            child_end += 1;
        }

        let children = if child_start < child_end {
            parse_override_range(lines, child_start, child_end, depth + 1)
        } else {
            Vec::new()
        };

        result.push(OverrideNode {
            node_type,
            name,
            value,
            children,
        });
        index = child_end;
    }

    result
}

fn indentation(line: &str) -> usize {
    line.len() - line.trim_start_matches('\t').len()
}

fn parse_override_line(trimmed: &str) -> (String, String, Option<String>) {
    let trimmed = trimmed.trim_start_matches('\u{feff}');
    if let Some(eq_pos) = trimmed.find(" = ").or_else(|| {
        if trimmed.ends_with(" =") {
            Some(trimmed.len() - 2)
        } else {
            None
        }
    }) {
        let before = &trimmed[..eq_pos];
        let after = if eq_pos + 3 <= trimmed.len() {
            &trimmed[eq_pos + 3..]
        } else {
            ""
        };
        let mut parts = before.splitn(2, ' ');
        let node_type = parts.next().unwrap_or_default().to_string();
        let name = parts.next().unwrap_or_default().to_string();
        return (node_type, name, Some(after.to_string()));
    }

    let mut parts = trimmed.splitn(2, ' ');
    let node_type = parts.next().unwrap_or_default().to_string();
    let name = parts.next().unwrap_or_default().to_string();
    (node_type, name, None)
}

pub fn serialize_override_tree(nodes: &[OverrideNode]) -> String {
    serialize_override_tree_at_depth(nodes, 0)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn find_first_node<'a, F>(nodes: &'a [OverrideNode], predicate: &F) -> Option<&'a OverrideNode>
where
    F: Fn(&OverrideNode) -> bool,
{
    nodes
        .iter()
        .find_map(|node| node.find_descendant(predicate))
}

pub fn find_first_node_mut<'a, F>(
    nodes: &'a mut [OverrideNode],
    predicate: &F,
) -> Option<&'a mut OverrideNode>
where
    F: Fn(&OverrideNode) -> bool,
{
    for node in nodes {
        if let Some(found) = node.find_descendant_mut(predicate) {
            return Some(found);
        }
    }

    None
}

fn serialize_override_tree_at_depth(nodes: &[OverrideNode], depth: usize) -> String {
    let indent = "\t".repeat(depth);
    let mut out = String::new();

    for node in nodes {
        if let Some(value) = &node.value {
            out.push_str(&format!(
                "{indent}{} {} = {}\n",
                node.node_type, node.name, value
            ));
        } else {
            out.push_str(&format!("{indent}{} {}\n", node.node_type, node.name));
        }
        out.push_str(&serialize_override_tree_at_depth(&node.children, depth + 1));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{OverrideNode, parse_override_text, serialize_override_tree};

    const SAMPLE_OVERRIDE: &str = "GameObject Background_Cave_01_SET 1\n\tGameObject FGLayer\n\t\tComponent UnityEngine.Transform\n\t\t\tVector3 m_LocalPosition\n\t\t\t\tFloat x = 1.5\n\t\t\t\tFloat y = 2.5\n\t\tGameObject Fill1\n\t\t\tComponent UnityEngine.Transform\n\t\t\t\tVector3 m_LocalPosition\n\t\t\t\t\tFloat y = 151.5137\n";

    #[test]
    fn parses_nested_unity_override_tree() {
        let nodes = parse_override_text(SAMPLE_OVERRIDE);
        assert_eq!(nodes.len(), 1);

        let root = &nodes[0];
        assert_eq!(root.node_type, "GameObject");
        assert_eq!(root.name, "Background_Cave_01_SET 1");

        let fg = root
            .child("GameObject", "FGLayer")
            .expect("missing FGLayer");
        let transform = fg
            .component("Transform")
            .expect("missing Transform component");
        let local_position = transform
            .child("Vector3", "m_LocalPosition")
            .expect("missing local position");
        assert_eq!(
            local_position
                .child("Float", "x")
                .and_then(OverrideNode::value_as_f32),
            Some(1.5)
        );
        assert_eq!(
            local_position
                .child("Float", "y")
                .and_then(OverrideNode::value_as_f32),
            Some(2.5)
        );
    }

    #[test]
    fn serializes_tree_back_to_original_text() {
        let nodes = parse_override_text(SAMPLE_OVERRIDE);
        assert_eq!(serialize_override_tree(&nodes), SAMPLE_OVERRIDE);
    }
}

use layout::adt::dag::NodeHandle;
use layout::backends::svg::SVGWriter;
use layout::core::base::Orientation;
use layout::core::geometry::Point;
use layout::core::style::LineStyleKind;
use layout::core::style::StyleAttr;
use layout::std_shapes::shapes::Arrow;
use layout::std_shapes::shapes::LineEndKind;
use layout::std_shapes::shapes::RecordDef;
use layout::std_shapes::shapes::ShapeKind;
use wasm_bindgen::prelude::wasm_bindgen;

use layout::std_shapes::shapes::Element;
use layout::topo::layout::VisualGraph;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub fn tree(config: &Config, data: &str) -> String {
    let mut root = Edge::default();

    let data = data
        .split_whitespace()
        .map(|line| line.chars().collect::<Vec<_>>())
        .collect::<Vec<_>>();

    for line in &data {
        insert(config, &mut root, line);
    }

    sort_edge(&mut root);

    let mut graph = VisualGraph::new(Orientation::TopToBottom);

    let node = graph.add_node(Element::create(
        ShapeKind::Box(String::new()),
        StyleAttr::simple(),
        Orientation::TopToBottom,
        Point::zero(),
    ));

    convert_edge(config, &root, node, &mut String::new(), &mut graph);

    let mut svg = SVGWriter::new();
    graph.do_it(false, false, false, &mut svg);
    let mut svg = svg.finalize();
    let i = svg.find("<svg").unwrap();
    svg.replace_range(0..i, "");
    svg
}

#[derive(Default)]
struct Node<'a> {
    edges: Vec<(char, Edge<'a>)>,
}

#[derive(Default)]
struct Edge<'a> {
    chars: &'a [char],
    child: Option<Node<'a>>,
}

#[wasm_bindgen(getter_with_clone)]
pub struct Config {
    max_edge_len: usize,
    node_sizes: Vec<usize>,
}

#[wasm_bindgen]
impl Config {
    #[wasm_bindgen(constructor)]
    pub fn new(max_edge_len: usize, node_sizes: Vec<usize>) -> Self {
        Self {
            max_edge_len,
            node_sizes,
        }
    }
}

fn insert<'a>(config: &Config, root: &mut Edge<'a>, key: &'a [char]) {
    if key.is_empty() {
        return;
    };

    match key.iter().zip(root.chars).position(|(k, e)| k != e) {
        // Full match
        None => {
            let len = key.len().min(root.chars.len());
            let Some(char) = key.get(len) else {
                // Note: violates prefix precondition
                return;
            };

            let key = &key[len + 1..];

            match root
                .child
                .iter_mut()
                .flat_map(|node| node.edges.iter_mut())
                .find_map(|(c, child)| if c == char { Some(child) } else { None })
            {
                None => root
                    .child
                    .get_or_insert_default()
                    .edges
                    .push((*char, path(config, key))),
                Some(child) => insert(config, child, key),
            }
        }
        // Partial match
        Some(i) => {
            let child = core::mem::take(&mut root.child);

            root.child = Some(Node {
                edges: vec![
                    (
                        root.chars[i],
                        Edge {
                            chars: &root.chars[i + 1..],
                            child,
                        },
                    ),
                    (key[i], path(config, &key[i + 1..])),
                ],
            });
            root.chars = &root.chars[..i];
        }
    }
}

fn path<'a>(config: &Config, key: &'a [char]) -> Edge<'a> {
    if key.len() > config.max_edge_len {
        Edge {
            chars: &key[..config.max_edge_len],
            child: Some(Node {
                edges: vec![(
                    key[config.max_edge_len],
                    path(config, &key[config.max_edge_len + 1..]),
                )],
            }),
        }
    } else {
        Edge {
            chars: key,
            child: None,
        }
    }
}

fn sort_edge(root: &mut Edge) {
    if let Some(node) = &mut root.child {
        sort_node(node);
    }
}

fn sort_node(node: &mut Node) {
    node.edges.sort_unstable_by_key(|(c, _)| *c);
    node.edges.iter_mut().for_each(|(_, edge)| sort_edge(edge));
}

fn convert_edge(
    config: &Config,
    root: &Edge,
    parent: NodeHandle,
    stack: &mut String,
    graph: &mut VisualGraph,
) {
    root.chars.iter().for_each(|c| stack.push(*c));
    let child = convert_node(config, root.child.as_ref(), stack, graph);
    root.chars.iter().map(|_| stack.pop()).count();

    let arrow = Arrow::new(
        LineEndKind::None,
        LineEndKind::Arrow,
        LineStyleKind::Normal,
        &root.chars.iter().collect::<String>(),
        &StyleAttr::simple(),
        &Some(stack.clone()),
        &None,
    );

    graph.add_edge(arrow, parent, child);
}

fn convert_node(
    config: &Config,
    root: Option<&Node>,
    stack: &mut String,
    graph: &mut VisualGraph,
) -> NodeHandle {
    let (shape, len) = match root {
        None => (ShapeKind::Circle(String::new()), 0),
        Some(root) => {
            let len = config
                .node_sizes
                .iter()
                .copied()
                .find(|size| *size >= root.edges.len())
                .unwrap_or(root.edges.len());

            let mut edges = 0u32;
            root.edges
                .iter()
                .for_each(|(c, _)| edges |= 1 << (*c as u32).saturating_sub('a' as u32).min(26));

            let mut record = if config.node_sizes.len() > 1 {
                vec![RecordDef::Text(format!("Node{}", len), None)]
            } else {
                Vec::new()
            };

            let mut offset = 0;
            while edges > 0 {
                let i = edges.trailing_zeros();

                if let Some(prev) = i.checked_sub(1) {
                    let lo = offset_to_alpha(offset as u8);
                    let hi = offset_to_alpha((offset + prev) as u8);
                    record.push(RecordDef::Text(
                        if lo == hi {
                            String::from(lo)
                        } else {
                            format!("{lo}..={hi}")
                        },
                        None,
                    ))
                }

                let c = offset_to_alpha((i + offset) as u8);
                record.push(RecordDef::Text(
                    String::from(c),
                    Some(format!("{stack}{c}")),
                ));

                offset += i + 1;
                edges >>= i + 1;
            }

            if offset < 26 {
                let lo = offset_to_alpha(offset as u8);
                let hi = offset_to_alpha(25);
                record.push(RecordDef::Text(
                    if lo == hi {
                        String::from(lo)
                    } else {
                        format!("{lo}..={hi}")
                    },
                    None,
                ))
            }

            let len = record.len();
            (ShapeKind::Record(RecordDef::Array(record)), len)
        }
    };

    let parent = Element::create(
        shape,
        StyleAttr::simple(),
        Orientation::LeftToRight,
        Point::new((50 * len) as f64, 50.),
    );

    let parent = graph.add_node(parent);

    if let Some(root) = root {
        root.edges.iter().for_each(|(c, edge)| {
            stack.push(*c);
            convert_edge(config, edge, parent, stack, graph);
            stack.pop();
        });
    }

    parent
}

fn offset_to_alpha(offset: u8) -> char {
    (b'a' + offset.min(25)) as char
}

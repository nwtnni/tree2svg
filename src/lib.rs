use layout::adt::dag::NodeHandle;
use layout::backends::svg::SVGWriter;
use layout::core::base::Orientation;
use layout::core::color::Color;
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
    let mut step = None;

    data.lines()
        .take(config.step.unwrap_or(usize::MAX))
        .map(|line| match line.split_once('=') {
            None => (line, None),
            Some((key, value)) => (key, value.trim().parse::<i32>().ok()),
        })
        .map(|(key, value)| (key.trim(), value))
        .filter(|(key, _)| !key.is_empty())
        .filter(|(key, _)| key.chars().all(|char| char.is_ascii_lowercase()))
        .map(|(key, value)| (key.as_bytes(), value))
        .for_each(|(key, value)| {
            // Highlight last step
            step = Some(key);
            insert(config, &mut root, key, value);
        });

    sort_edge(&mut root);

    let mut graph = VisualGraph::new(Orientation::TopToBottom);

    let node = graph.add_node(Element::create(
        ShapeKind::Box(String::new()),
        StyleAttr::simple(),
        Orientation::TopToBottom,
        Point::zero(),
    ));

    render_edge(config, step, &root, node, &mut String::new(), &mut graph);

    let mut svg = SVGWriter::new();
    graph.do_it(false, false, false, &mut svg);
    let mut svg = svg.finalize();
    let i = svg.find("<svg").unwrap();
    svg.replace_range(0..i, "");
    svg
}

#[derive(Default)]
struct Node<'a> {
    edges: Vec<(u8, Edge<'a>)>,
}

#[derive(Default)]
struct Edge<'a> {
    chars: &'a [u8],
    child: Child<'a>,
}

enum Child<'a> {
    Node(Node<'a>),
    Value(Option<i32>),
}

impl<'a> Child<'a> {
    fn push(&mut self, char: u8, edge: Edge<'a>) {
        match self {
            Self::Node(node) => {
                node.edges.push((char, edge));
            }
            Self::Value(_) => {
                *self = Self::Node(Node {
                    edges: vec![(char, edge)],
                })
            }
        }
    }
}

impl Default for Child<'_> {
    fn default() -> Self {
        Self::Value(None)
    }
}

#[wasm_bindgen(getter_with_clone)]
pub struct Config {
    edge: usize,
    node: Vec<usize>,
    step: Option<usize>,
}

#[wasm_bindgen]
impl Config {
    #[wasm_bindgen(constructor)]
    pub fn new(edge: usize, node: Vec<usize>) -> Self {
        Self {
            edge,
            node,
            step: None,
        }
    }

    #[wasm_bindgen(setter)]
    pub fn set_step(&mut self, step: Option<usize>) {
        self.step = step;
    }

    #[wasm_bindgen(setter)]
    pub fn set_edge(&mut self, edge: usize) {
        self.edge = edge;
    }

    #[wasm_bindgen(setter)]
    pub fn set_node(&mut self, node: Vec<usize>) {
        self.node = node;
    }
}

fn insert<'a>(config: &Config, root: &mut Edge<'a>, key: &'a [u8], value: Option<i32>) {
    match key.iter().zip(root.chars).position(|(k, e)| k != e) {
        // Full match
        None => {
            let len = key.len().min(root.chars.len());
            let Some(char) = key.get(len) else {
                // Note: violates prefix precondition
                // value overwrites node
                root.child = Child::Value(value);
                return;
            };

            match &mut root.child {
                Child::Value(None) => {
                    *root = path(config, key, value);
                    return;
                }
                // Note: violates prefix precondition
                // node overwrites value
                Child::Value(Some(_)) => (),
                Child::Node(node) => {
                    if let Some(child) = node
                        .edges
                        .iter_mut()
                        .find_map(|(c, child)| if c == char { Some(child) } else { None })
                    {
                        return insert(config, child, &key[len + 1..], value);
                    }
                }
            }

            root.child.push(*char, path(config, &key[len + 1..], value))
        }
        // Partial match
        Some(i) => {
            let child = core::mem::take(&mut root.child);

            root.child = Child::Node(Node {
                edges: vec![
                    (
                        root.chars[i],
                        Edge {
                            chars: &root.chars[i + 1..],
                            child,
                        },
                    ),
                    (key[i], path(config, &key[i + 1..], value)),
                ],
            });
            root.chars = &root.chars[..i];
        }
    }
}

fn path<'a>(config: &Config, key: &'a [u8], value: Option<i32>) -> Edge<'a> {
    if key.len() > config.edge {
        Edge {
            chars: &key[..config.edge],
            child: Child::Node(Node {
                edges: vec![(
                    key[config.edge],
                    path(config, &key[config.edge + 1..], value),
                )],
            }),
        }
    } else {
        Edge {
            chars: key,
            child: Child::Value(value),
        }
    }
}

fn sort_edge(root: &mut Edge) {
    if let Child::Node(node) = &mut root.child {
        sort_node(node);
    }
}

fn sort_node(node: &mut Node) {
    node.edges.sort_unstable_by_key(|(c, _)| *c);
    node.edges.iter_mut().for_each(|(_, edge)| sort_edge(edge));
}

fn render_edge(
    config: &Config,
    step: Option<&[u8]>,
    root: &Edge,
    parent: NodeHandle,
    stack: &mut String,
    graph: &mut VisualGraph,
) {
    root.chars.iter().for_each(|c| stack.push(*c as char));
    let child = render_child(config, step, &root.child, stack, graph);
    root.chars.iter().map(|_| stack.pop()).count();

    let arrow = Arrow::new(
        LineEndKind::None,
        LineEndKind::Arrow,
        LineStyleKind::Normal,
        &root
            .chars
            .iter()
            .map(|byte| *byte as char)
            .collect::<String>(),
        &StyleAttr {
            line_color: if step.is_some_and(|key| key.starts_with(stack.as_bytes())) {
                Color::fast("green")
            } else {
                Color::fast("black")
            },
            ..StyleAttr::simple()
        },
        &Some(stack.clone()),
        &None,
    );

    graph.add_edge(arrow, parent, child);
}

fn render_child(
    config: &Config,
    step: Option<&[u8]>,
    root: &Child,
    stack: &mut String,
    graph: &mut VisualGraph,
) -> NodeHandle {
    let (shape, len) = match root {
        Child::Value(None) => (ShapeKind::Circle(String::new()), 0),
        Child::Value(Some(value)) => (ShapeKind::Circle(value.to_string()), 1),
        Child::Node(root) => {
            let len = config
                .node
                .iter()
                .copied()
                .find(|size| *size >= root.edges.len())
                .unwrap_or(26);

            let mut edges = 0u32;
            root.edges
                .iter()
                .for_each(|(c, _)| edges |= 1 << (*c - b'a'));

            let mut record = if config.node.is_empty() {
                Vec::new()
            } else {
                vec![RecordDef::Text(format!("Node{}", len), None)]
            };

            let mut offset = 0;
            while edges > 0 {
                let i = edges.trailing_zeros();

                if len == 26
                    && let Some(prev) = i.checked_sub(1)
                {
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

            if len == 26 {
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
            } else {
                record.extend(
                    (0..len - root.edges.len()).map(|_| RecordDef::Text(String::new(), None)),
                )
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

    if let Child::Node(root) = root {
        root.edges.iter().for_each(|(c, edge)| {
            stack.push(*c as char);
            render_edge(config, step, edge, parent, stack, graph);
            stack.pop();
        });
    }

    parent
}

fn offset_to_alpha(offset: u8) -> char {
    (b'a' + offset.min(25)) as char
}

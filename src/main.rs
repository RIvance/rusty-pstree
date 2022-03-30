extern crate clap;
extern crate ptree;
extern crate regex;


use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;

use clap::Parser;
use ptree::Color;
use regex::Regex;
use ptree::TreeBuilder;
use ptree::PrintConfig;


#[derive(Clone)]
struct ProcessInfo
{
    pid: u32,
    ppid: u32,
    name: String,
}

type ProcessNodeRef = Rc<RefCell<ProcessNode>>;

struct ProcessTree
{
    pub root: ProcessNodeRef,
}

impl ProcessTree 
{
    pub fn new(root: &ProcessNodeRef) -> ProcessTree
    {
        ProcessTree { root: ProcessNodeRef::clone(root) }
    }
}

struct ProcessNode
{
    pub proc_info: ProcessInfo,
    pub children: Vec<ProcessNodeRef>,
}

struct PsTreePrintConfig
{
    show_pid: bool,
    root_pid: u32,
    print_config: PrintConfig,
}

impl ProcessTree
{
    pub fn print(&self, config: &PsTreePrintConfig)
    {
        let mut stack: Vec<(ProcessNodeRef, i32)> = vec![(Rc::clone(&self.root), 0)];

        let mut tree_builder = TreeBuilder::new(String::new());

        while !stack.is_empty() {

            let node_depth_entry = stack.pop().unwrap();
            let node = node_depth_entry.0.try_borrow().unwrap();
            let depth = node_depth_entry.1;

            let proc_info = node.proc_info.clone();

            let node_str = if config.show_pid {
                format!("[{}] {}", proc_info.pid, proc_info.name)
            } else {
                proc_info.name
            };

            if depth == 0 {
                tree_builder = TreeBuilder::new(node_str);
            } else if node.children_count() > 0 {
                tree_builder.begin_child(node_str);
            } else {
                tree_builder.add_empty_child(node_str);
            }

            for child in node.children.iter().rev() {
                stack.push((Rc::clone(&child), depth + 1));
            }

            if !stack.is_empty() {
                let next_depth = stack.last().unwrap().1;
                for _ in 0 .. depth - next_depth {
                    tree_builder.end_child();
                }
            }
        }

        let tree = tree_builder.build();
        let _ = ptree::print_tree_with(&tree, &config.print_config);
    }

    pub fn filter_unique(&mut self)
    {
        let mut stack: Vec<ProcessNodeRef> = vec![Rc::clone(&self.root)];

        while !stack.is_empty() {
            let node_ref = stack.pop().unwrap();
            node_ref.borrow_mut().children.dedup_by(|p1, p2| {
                p1.borrow().children.len() == 0 && 
                p1.borrow().children.len() == 0 && 
                p1.borrow().proc_info.name == p2.borrow().proc_info.name
            });
            node_ref.borrow().children.iter().for_each(|child| stack.push(Rc::clone(child)));
        }
    }
}

impl ProcessNode 
{
    pub fn new(proc_info: ProcessInfo) -> ProcessNode
    {
        ProcessNode 
        { 
            proc_info, 
            children: Vec::new(),
        }
    }

    pub fn to_heap(self) -> ProcessNodeRef
    {
        Rc::new(RefCell::new(self))
    }

    pub fn add_child(&mut self, child: ProcessNodeRef)
    {
        self.children.push(child);
    }

    pub fn children_count(&self) -> usize
    {
        self.children.len()
    }

}

impl PsTreePrintConfig 
{
    pub fn new() -> PsTreePrintConfig
    {
        PsTreePrintConfig 
        { 
            show_pid: false,
            root_pid: 0,
            print_config: PrintConfig::default(),
        }
    }

}

fn parse_proc_stat(stat: &str) -> ProcessInfo
{
    let regex = Regex::new(r"(Name:\s*(?P<name>.+)\n)([\s\S]*)(Pid:\s*(?P<pid>\d+))([\s\S]*)(PPid:\s*(?P<ppid>\d+))").unwrap();
    let capture = regex.captures_iter(stat).next().unwrap();

    let pid = str::parse::<u32>(&capture["pid"]).unwrap();
    let ppid = str::parse::<u32>(&capture["ppid"]).unwrap();
    let name = capture["name"].to_string();

    ProcessInfo { pid, ppid, name }
}

fn get_process_info() -> Vec<ProcessInfo>
{
    let proc_path_iter = match fs::read_dir("/proc") {
        Ok(proc_dir) => {
            proc_dir.map(Result::unwrap).filter(|path| {
                path.file_name().into_string().unwrap().chars().all(char::is_numeric)
            })
        }
        Err(why) => panic!("Unable to open \"/proc\": {}", why)
    };

    let mut proc_vec: Vec<ProcessInfo> = Vec::new();

    for mut proc_path in proc_path_iter.map(|dir| dir.path()) {
        proc_path.push("status");
        if let Ok(proc_stat) = fs::read_to_string(proc_path) {
            proc_vec.push(parse_proc_stat(&proc_stat));
        }
    }

    proc_vec.sort_by(|p0, p1| p0.pid.cmp(&p1.pid));
    return proc_vec;
}

fn treefy_proc(proc_info_vec: Vec<ProcessInfo>, root_pid: u32) -> ProcessTree
{
    let mut node_map: HashMap<u32, ProcessNodeRef> = HashMap::new();

    let first_proc = if root_pid == 0 {
        proc_info_vec.first().unwrap().clone()
    } else {
        proc_info_vec.iter()
            .find(|&proc| proc.pid == root_pid)
            .expect(&format!("process {} does not exist", root_pid))
            .clone()
    };

    for proc_info in proc_info_vec {
        let node_ptr = ProcessNode::new(proc_info.clone()).to_heap();
        if proc_info.ppid != 0 {
            node_map.entry(proc_info.ppid).and_modify(|node| {
                node.borrow_mut().add_child(Rc::clone(&node_ptr));
            });
        }
        node_map.insert(proc_info.pid, node_ptr);
    }

    return ProcessTree::new(node_map.get(&first_proc.pid).unwrap());
}

#[derive(Parser)]
#[clap(version)]
struct Args
{
    /// node color, a string in ["white", "red", "green", ...] or an RGB triple like "255,255,0"
    #[clap(short = 'c', long)]
    node_color: Option<String>,

    /// branch color, a string in ["white", "red", "green", ...] or an RGB triple like "255,255,0"
    #[clap(short, long)]
    branch_color: Option<String>,

    /// node background color, a string in ["white", "red", "green", ...] or an RGB triple like "255,255,0"
    #[clap(short = 'g', long)]
    background: Option<String>,

    /// show pid
    #[clap(short = 'p', long)]
    show_pid: bool,

    /// remove the duplicated leaf node 
    #[clap(short, long)]
    unique: bool,

    /// show the process tree rooted on a specific pid
    #[clap(short, long, default_value = "0")]
    root_pid: u32,

    #[clap(short, long)]
    depth: Option<u32>,

}

fn parse_rgb(rgb_str: &str) -> Option<Color>
{
    let regex = Regex::new(r"(?P<r>\d+),(?P<g>\d+),(?P<b>\d+)").unwrap();
    let capture = regex.captures_iter(&rgb_str).next().unwrap();
    let mut rgb: [u8; 3] = [0, 0, 0];

    for (i, color) in "rgb".chars().enumerate() {
        rgb[i] = match str::parse::<u8>(&capture[String::from(color).as_str()]) {
            Ok(color) => color,
            Err(_) => return None,
        };
    }

    Some(Color::RGB(rgb[0], rgb[1], rgb[2]))
}

fn parse_color(color_str: &str) -> Option<Color>
{
    match color_str.to_lowercase().as_str() {
        "black"   => Some(Color::Black),
        "red"     => Some(Color::Red),
        "green"   => Some(Color::Green),
        "yellow"  => Some(Color::Yellow),
        "blue"    => Some(Color::Blue),
        "purple"  => Some(Color::Purple),
        "cyan"    => Some(Color::Cyan),
        "white"   => Some(Color::White),
        _ => parse_rgb(color_str),
    }
}

fn parse_config(args: Args) -> PsTreePrintConfig
{
    let mut config = PsTreePrintConfig::new();

    config.print_config.leaf.foreground = args.node_color.and_then(|color_str| parse_color(&color_str));
    config.print_config.leaf.background = args.background.and_then(|color_str| parse_color(&color_str));
    config.print_config.branch.foreground = args.branch_color.and_then(|color_str| parse_color(&color_str));

    config.show_pid = args.show_pid;
    config.root_pid = args.root_pid;

    if let Some(val) = args.depth {
        config.print_config.depth = val;
    }

    return config;
}

fn main()
{
    let ps_info = get_process_info();
    let args = Args::parse();
    let mut pstree = treefy_proc(ps_info, args.root_pid);
    args.unique.then(|| pstree.filter_unique());
    pstree.print(&parse_config(args));
}

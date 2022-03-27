extern crate ptree;
extern crate regex;

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;

use ptree::{TreeBuilder, print_tree};
use regex::Regex;

struct ProcessInfo
{
    pid: i32,
    ppid: i32,
    name: String,
}

impl Clone for ProcessInfo 
{
    fn clone(&self) -> Self 
    {
        Self 
        {
            pid: self.pid.clone(), 
            ppid: self.ppid.clone(), 
            name: self.name.clone() 
        }
    }
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

impl ProcessTree
{
    pub fn print(&self)
    {
        let mut stack: Vec<(ProcessNodeRef, i32)> = vec![(Rc::clone(&self.root), 0)];

        let mut tree_builder = TreeBuilder::new("scheduler".to_string());

        while !stack.is_empty() {
            let node_depth_entry = stack.pop().unwrap();
            let node = node_depth_entry.0;
            let depth = node_depth_entry.1;

            if node.borrow().children.len() > 0 {
                tree_builder.begin_child(node.borrow().proc_info.name.clone());
            } else {
                tree_builder.add_empty_child(node.borrow().proc_info.name.clone());
            }
            for child in node.borrow().children.iter().rev() {
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
        let _ = print_tree(&tree);
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
}

fn parse_proc_stat(stat: &str) -> ProcessInfo
{
    let re = Regex::new(r"(Name:\s*(?P<name>.+)\n)([\s\S]*)(Pid:\s*(?P<pid>\d+))([\s\S]*)(PPid:\s*(?P<ppid>\d+))").unwrap();
    let capture = re.captures_iter(stat).next().unwrap();

    let pid = str::parse::<i32>(&capture["pid"]).unwrap();
    let ppid = str::parse::<i32>(&capture["ppid"]).unwrap();
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

fn treefy_proc(proc_info_vec: Vec<ProcessInfo>) -> ProcessTree
{
    let mut node_map: HashMap<i32, ProcessNodeRef> = HashMap::new();

    let first_proc = proc_info_vec.first().unwrap().clone();

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

fn main() 
{
    let ps_info = get_process_info();
    let pstree = treefy_proc(ps_info);
    pstree.print();
}

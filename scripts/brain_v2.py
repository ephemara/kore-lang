#!/usr/bin/env python3
"""
K_OS Brain v2 - Multi-Modal Semantic Understanding
==================================================
Features:
- AST-aware semantic chunking (functions, classes, modules)
- Code dependency graph for cross-referencing
- Intent recognition for query understanding
- Hybrid search pipeline (vector + keyword + structure)
- Smart caching and predictive indexing
"""

import os
import sys
import time
import threading
import logging
import ast
import re
import json
import subprocess
from pathlib import Path
from typing import List, Optional, Dict, Any, Tuple, Set
from collections import defaultdict, deque
from dataclasses import dataclass, asdict
from dotenv import load_dotenv

# Search for .env.local in parents
root_dir = Path(__file__).parent.parent.parent.parent
load_dotenv(root_dir / ".env.local")

# Ensure we can import from parent if run directly
current_dir = Path(__file__).parent
if str(current_dir.parent.parent) not in sys.path:
    sys.path.append(str(current_dir.parent.parent))

from kos.api import register
from kos.scripts.privacy import PrivacyGuard

# Configure logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(name)s - %(levelname)s - %(message)s')
logger = logging.getLogger("BrainV2")

# Force UTF-8 for Windows console output
if sys.platform == "win32":
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')

try:
    import lancedb
    import numpy as np
    from watchdog.observers import Observer
    from watchdog.events import FileSystemEventHandler
    from openai import OpenAI
    from google import genai
    import pyarrow as pa
    
    # Try FastEmbed for ONNX
    try:
        from fastembed.embedding import TextEmbedding
        HAS_FASTEMBED = True
    except ImportError:
        HAS_FASTEMBED = False
        # Fallback
        from sentence_transformers import SentenceTransformer

except ImportError as e:
    logger.error(f"Missing dependencies: {e}")
    FileSystemEventHandler = object
    HAS_FASTEMBED = False

# --- DATA STRUCTURES ---

@dataclass
class CodeEntity:
    """Represents a semantic code unit (function, class, module, etc.)"""
    id: str
    type: str  # 'function', 'class', 'method', 'module', 'struct', 'impl'
    name: str
    content: str
    docstring: Optional[str]
    signature: Optional[str]
    file_path: str
    start_line: int
    end_line: int
    imports: List[str]
    exports: List[str]
    dependencies: List[str]
    complexity: int
    vector: Optional[List[float]] = None
    metadata: Dict[str, Any] = None

@dataclass
class QueryIntent:
    """Parsed intent from user query"""
    intent_type: str  # 'find_code', 'explain', 'debug', 'refactor', 'architect'
    entities: List[str]  # Function/class names mentioned
    concepts: List[str]  # Technical concepts
    context: str  # 'implementation', 'design', 'testing', 'performance'
    specificity: float  # 0-1, how specific vs general

@dataclass
class SearchResult:
    """Enhanced search result with relevance scoring"""
    entity: CodeEntity
    score: float
    reason: str  # Why this was matched
    related: List[str]  # Related entities

# --- AST PARSERS ---

class ASTCodeParser:
    """Multi-language AST parser for semantic understanding"""
    
    # Language-specific patterns
    EXTENSION_PARSERS = {
        '.py': 'parse_python',
        '.rs': 'parse_rust',
        '.ts': 'parse_typescript',
        '.tsx': 'parse_typescript',
        '.js': 'parse_javascript',
    }
    
    def __init__(self):
        self.import_patterns = {
            '.py': r'^from\s+(\S+)\s+import|^import\s+(\S+)',
            '.rs': r'^use\s+([^;]+);',
            '.ts': r'^import\s+.*from\s+[\'"]([^\'"]+)[\'"]|^import\s+[\'"]([^\'"]+)[\'"]',
            '.tsx': r'^import\s+.*from\s+[\'"]([^\'"]+)[\'"]|^import\s+[\'"]([^\'"]+)[\'"]',
            '.js': r'^import\s+.*from\s+[\'"]([^\'"]+)[\'"]|^import\s+[\'"]([^\'"]+)[\'"]',
        }
    
    def parse_file(self, file_path: str, content: str) -> List[CodeEntity]:
        """Parse file into semantic entities"""
        ext = Path(file_path).suffix
        if ext not in self.EXTENSION_PARSERS:
            return self._fallback_chunk(file_path, content)
        
        try:
            parser_method = getattr(self, self.EXTENSION_PARSERS[ext])
            return parser_method(file_path, content)
        except Exception as e:
            logger.warning(f"AST parse failed for {file_path}: {e}")
            return self._fallback_chunk(file_path, content)
    
    def parse_python(self, file_path: str, content: str) -> List[CodeEntity]:
        """Parse Python code using AST"""
        entities = []
        try:
            tree = ast.parse(content)
            lines = content.splitlines()
            
            for node in ast.walk(tree):
                if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    # Extract function
                    start_line = node.lineno
                    end_line = node.end_lineno if hasattr(node, 'end_lineno') else start_line
                    docstring = ast.get_docstring(node)
                    
                    # Get signature
                    args = [arg.arg for arg in node.args.args]
                    signature = f"def {node.name}({', '.join(args)})"
                    
                    # Extract content
                    func_content = "\n".join(lines[start_line-1:end_line])
                    
                    # Find imports (naive - look at file level)
                    imports = self._extract_imports(content, '.py')
                    
                    entities.append(CodeEntity(
                        id=f"{file_path}:{start_line}",
                        type='function',
                        name=node.name,
                        content=func_content,
                        docstring=docstring,
                        signature=signature,
                        file_path=file_path,
                        start_line=start_line,
                        end_line=end_line,
                        imports=imports,
                        exports=[node.name],
                        dependencies=[],
                        complexity=self._calculate_complexity(func_content),
                        metadata={'decorators': [d.id for d in node.decorator_list if isinstance(d, ast.Name)]}
                    ))
                
                elif isinstance(node, ast.ClassDef):
                    # Extract class
                    start_line = node.lineno
                    end_line = node.end_lineno if hasattr(node, 'end_lineno') else start_line
                    docstring = ast.get_docstring(node)
                    
                    # Get methods
                    methods = []
                    for item in node.body:
                        if isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef)):
                            methods.append(item.name)
                    
                    class_content = "\n".join(lines[start_line-1:end_line])
                    imports = self._extract_imports(content, '.py')
                    
                    entities.append(CodeEntity(
                        id=f"{file_path}:{start_line}",
                        type='class',
                        name=node.name,
                        content=class_content,
                        docstring=docstring,
                        signature=f"class {node.name}",
                        file_path=file_path,
                        start_line=start_line,
                        end_line=end_line,
                        imports=imports,
                        exports=[node.name] + methods,
                        dependencies=[],
                        complexity=self._calculate_complexity(class_content),
                        metadata={'methods': methods, 'bases': [ast.unparse(b) for b in node.bases]}
                    ))
            
            # Module-level docstring
            if tree.body and isinstance(tree.body[0], ast.Expr) and isinstance(tree.body[0].value, ast.Constant):
                entities.append(CodeEntity(
                    id=f"{file_path}:0",
                    type='module',
                    name=Path(file_path).name,
                    content=content[:500],
                    docstring=str(tree.body[0].value.value),
                    signature=f"module {Path(file_path).stem}",
                    file_path=file_path,
                    start_line=0,
                    end_line=5,
                    imports=self._extract_imports(content, '.py'),
                    exports=[],
                    dependencies=[],
                    complexity=0,
                    metadata={'module_level': True}
                ))
            
        except Exception as e:
            logger.error(f"Python AST error: {e}")
            return self._fallback_chunk(file_path, content)
        
        return entities
    
    def parse_rust(self, file_path: str, content: str) -> List[CodeEntity]:
        """Parse Rust code using regex patterns (since Python AST doesn't support Rust)"""
        entities = []
        lines = content.splitlines()
        
        # Find functions
        func_pattern = r'(pub\s+)?(async\s+)?fn\s+(\w+)\s*\('
        for match in re.finditer(func_pattern, content):
            name = match.group(3)
            start_line = content[:match.start()].count('\n') + 1
            
            # Find end of function (naive - find closing brace)
            rest = content[match.start():]
            brace_count = 0
            end_pos = 0
            for i, char in enumerate(rest):
                if char == '{':
                    brace_count += 1
                elif char == '}':
                    brace_count -= 1
                    if brace_count == 0:
                        end_pos = i
                        break
            
            func_content = rest[:end_pos+1]
            end_line = start_line + func_content.count('\n')
            
            entities.append(CodeEntity(
                id=f"{file_path}:{start_line}",
                type='function',
                name=name,
                content=func_content,
                docstring=None,
                signature=f"fn {name}",
                file_path=file_path,
                start_line=start_line,
                end_line=end_line,
                imports=self._extract_imports(content, '.rs'),
                exports=[name],
                dependencies=[],
                complexity=self._calculate_complexity(func_content),
                metadata={'visibility': 'pub' if match.group(1) else 'private'}
            ))
        
        # Find structs
        struct_pattern = r'(pub\s+)?struct\s+(\w+)'
        for match in re.finditer(struct_pattern, content):
            name = match.group(2)
            start_line = content[:match.start()].count('\n') + 1
            
            entities.append(CodeEntity(
                id=f"{file_path}:{start_line}",
                type='struct',
                name=name,
                content=match.group(0),
                docstring=None,
                signature=f"struct {name}",
                file_path=file_path,
                start_line=start_line,
                end_line=start_line,
                imports=self._extract_imports(content, '.rs'),
                exports=[name],
                dependencies=[],
                complexity=0,
                metadata={'visibility': 'pub' if match.group(1) else 'private'}
            ))
        
        return entities
    
    def parse_typescript(self, file_path: str, content: str) -> List[CodeEntity]:
        """Parse TypeScript/JavaScript code"""
        entities = []
        
        # Functions
        func_pattern = r'(export\s+)?(async\s+)?function\s+(\w+)\s*\(|const\s+(\w+)\s*=\s*\([^)]*\)\s*=>'
        for match in re.finditer(func_pattern, content):
            name = match.group(3) or match.group(4)
            if not name:
                continue
                
            start_line = content[:match.start()].count('\n') + 1
            entities.append(CodeEntity(
                id=f"{file_path}:{start_line}",
                type='function',
                name=name,
                content=match.group(0),
                docstring=None,
                signature=f"function {name}",
                file_path=file_path,
                start_line=start_line,
                end_line=start_line,
                imports=self._extract_imports(content, '.ts'),
                exports=[name] if 'export' in match.group(0) else [],
                dependencies=[],
                complexity=self._calculate_complexity(match.group(0)),
                metadata={'arrow': '=>' in match.group(0)}
            ))
        
        # Classes
        class_pattern = r'(export\s+)?class\s+(\w+)'
        for match in re.finditer(class_pattern, content):
            name = match.group(2)
            start_line = content[:match.start()].count('\n') + 1
            
            entities.append(CodeEntity(
                id=f"{file_path}:{start_line}",
                type='class',
                name=name,
                content=match.group(0),
                docstring=None,
                signature=f"class {name}",
                file_path=file_path,
                start_line=start_line,
                end_line=start_line,
                imports=self._extract_imports(content, '.ts'),
                exports=[name] if 'export' in match.group(0) else [],
                dependencies=[],
                complexity=0,
                metadata={}
            ))
        
        return entities
    
    def parse_javascript(self, file_path: str, content: str) -> List[CodeEntity]:
        """Parse JavaScript (reuse TS parser)"""
        return self.parse_typescript(file_path, content)
    
    def _fallback_chunk(self, file_path: str, content: str) -> List[CodeEntity]:
        """Fallback: chunk by 50-line blocks"""
        entities = []
        lines = content.splitlines()
        
        for i in range(0, len(lines), 50):
            chunk = "\n".join(lines[i:i+50])
            if not chunk.strip():
                continue
                
            entities.append(CodeEntity(
                id=f"{file_path}:{i}",
                type='chunk',
                name=f"chunk_{i//50}",
                content=chunk,
                docstring=None,
                signature=None,
                file_path=file_path,
                start_line=i,
                end_line=min(i+50, len(lines)),
                imports=self._extract_imports(content, Path(file_path).suffix),
                exports=[],
                dependencies=[],
                complexity=self._calculate_complexity(chunk),
                metadata={'fallback': True}
            ))
        
        return entities
    
    def _extract_imports(self, content: str, ext: str) -> List[str]:
        """Extract imports/dependencies"""
        if ext not in self.import_patterns:
            return []
        
        pattern = self.import_patterns[ext]
        imports = []
        for line in content.splitlines():
            match = re.match(pattern, line.strip())
            if match:
                # Get first non-None group
                imp = next(g for g in match.groups() if g)
                imports.append(imp)
        return imports
    
    def _calculate_complexity(self, content: str) -> int:
        """Calculate basic complexity score"""
        score = 0
        # Count control flow
        score += len(re.findall(r'\b(if|else|for|while|match|try|catch|except)\b', content))
        # Count operators
        score += len(re.findall(r'[+\-*/%=&|!]{2,}', content))
        # Count nesting (rough)
        score += content.count('{') * 2
        return score

# --- INTENT RECOGNITION ---

class IntentRecognizer:
    """Recognizes user intent from queries"""
    
    def __init__(self):
        self.intent_patterns = {
            'find_code': [
                r'\b(find|search|locate|where|show|get)\b.*\b(code|function|class|struct|impl|method)\b',
                r'\b(example|snippet|reference)\b',
                r'\b(how to|how do)\b.*\b(implement|use|call)\b'
            ],
            'explain': [
                r'\b(explain|what is|how does|describe)\b',
                r'\b(understand|clarify|meaning)\b',
                r'\b(why|how)\b.*\b(work|function|system)\b'
            ],
            'debug': [
                r'\b(error|bug|issue|fail|crash|broken)\b',
                r'\b(fix|solve|resolve|debug)\b',
                r'\b(not working|wrong|incorrect)\b'
            ],
            'refactor': [
                r'\b(refactor|improve|optimize|clean|simplify)\b',
                r'\b(better|efficient|performance)\b',
                r'\b(restructure|reorganize)\b'
            ],
            'architect': [
                r'\b(architecture|design|pattern|structure)\b',
                r'\b(system|module|component|service)\b',
                r'\b(how should|best way|approach)\b'
            ]
        }
        
        self.concept_keywords = {
            'rendering': ['render', 'shader', 'pipeline', 'gpu', 'wgpu', 'draw', 'vertex', 'fragment'],
            'memory': ['memory', 'alloc', 'buffer', 'cache', 'leak', 'ownership', 'borrow'],
            'concurrency': ['thread', 'async', 'await', 'sync', 'mutex', 'lock', 'channel'],
            'ui': ['ui', 'interface', 'widget', 'window', 'event', 'input', 'layout'],
            'data': ['data', 'struct', 'class', 'object', 'array', 'vector', 'map'],
            'network': ['network', 'http', 'socket', 'server', 'client', 'request'],
            'database': ['database', 'sql', 'query', 'table', 'index', 'lance', 'vector'],
            'ai': ['ai', 'ml', 'model', 'embedding', 'vector', 'semantic', 'search']
        }
    
    def recognize(self, query: str) -> QueryIntent:
        """Recognize intent from query"""
        query_lower = query.lower()
        
        # Determine intent type
        intent_type = 'find_code'  # default
        max_score = 0
        
        for intent, patterns in self.intent_patterns.items():
            score = sum(1 for pattern in patterns if re.search(pattern, query_lower))
            if score > max_score:
                max_score = score
                intent_type = intent
        
        # Extract entities (potential function/class names)
        entities = re.findall(r'\b([A-Z][a-zA-Z0-9_]*)\b', query)
        entities += re.findall(r'\b([a-z][a-zA-Z0-9_]*)\b.*\(\)', query)
        
        # Extract concepts
        concepts = []
        for concept, keywords in self.concept_keywords.items():
            if any(keyword in query_lower for keyword in keywords):
                concepts.append(concept)
        
        # Determine context
        context = 'implementation'
        if any(word in query_lower for word in ['design', 'architecture', 'pattern']):
            context = 'design'
        elif any(word in query_lower for word in ['error', 'bug', 'fix', 'debug']):
            context = 'debugging'
        elif any(word in query_lower for word in ['performance', 'optimize', 'fast']):
            context = 'performance'
        
        # Calculate specificity
        specificity = min(1.0, len(entities) * 0.3 + len(concepts) * 0.2 + len(query.split()) * 0.05)
        
        return QueryIntent(
            intent_type=intent_type,
            entities=entities,
            concepts=concepts,
            context=context,
            specificity=specificity
        )

# --- CODE GRAPH ---

class CodeGraph:
    """Builds and maintains code dependency graph"""
    
    def __init__(self):
        self.nodes = {}  # id -> CodeEntity
        self.edges = defaultdict(list)  # node_id -> [related_node_ids]
        self.import_map = defaultdict(list)  # import_name -> [node_ids]
        self.reverse_deps = defaultdict(list)  # node_id -> [nodes_that_depend_on_it]
    
    def add_entity(self, entity: CodeEntity):
        """Add entity to graph"""
        self.nodes[entity.id] = entity
        
        # Map imports
        for imp in entity.imports:
            self.import_map[imp].append(entity.id)
        
        # Find dependencies
        for imp in entity.imports:
            # Find entities that export this import
            for node_id, node in self.nodes.items():
                if imp in node.exports:
                    self.edges[entity.id].append(node_id)
                    self.reverse_deps[node_id].append(entity.id)
    
    def get_related(self, entity_id: str, depth: int = 2) -> List[str]:
        """Get related entities up to depth"""
        related = set()
        frontier = {entity_id}
        
        for _ in range(depth):
            new_frontier = set()
            for node_id in frontier:
                related.add(node_id)
                new_frontier.update(self.edges[node_id])
            frontier = new_frontier - related
        
        return list(related)
    
    def get_callers(self, entity_id: str) -> List[str]:
        """Get entities that depend on this one"""
        return self.reverse_deps.get(entity_id, [])
    
    def find_by_name(self, name: str) -> List[str]:
        """Find entities by name"""
        return [nid for nid, node in self.nodes.items() 
                if node.name.lower() == name.lower() or name.lower() in node.name.lower()]

# --- SMART CACHE ---

class SmartCache:
    """LRU cache for frequent queries"""
    
    def __init__(self, max_size: int = 100):
        self.cache = {}
        self.access_order = deque()
        self.max_size = max_size
        self.lock = threading.Lock()
    
    def get(self, key: str) -> Optional[List[SearchResult]]:
        with self.lock:
            if key in self.cache:
                # Move to end (most recently used)
                self.access_order.remove(key)
                self.access_order.append(key)
                return self.cache[key]
        return None
    
    def put(self, key: str, value: List[SearchResult]):
        with self.lock:
            if key in self.cache:
                self.access_order.remove(key)
            elif len(self.cache) >= self.max_size:
                # Remove least recently used
                lru_key = self.access_order.popleft()
                del self.cache[lru_key]
            
            self.cache[key] = value
            self.access_order.append(key)

# --- ENHANCED BRAIN ---

class BrainV2:
    def __init__(self):
        self.ready = False
        self.db = None
        self.table = None
        self.model = None
        self.tokenizer = None
        self.use_onnx = False
        self.observer = None
        self.openai_client = None  # OpenRouter client
        self.gemini_client = None  # Direct Gemini client
        
        # New components
        self.ast_parser = ASTCodeParser()
        self.intent_recognizer = IntentRecognizer()
        self.code_graph = CodeGraph()
        self.smart_cache = SmartCache()
        
        # State
        self.entity_cache = {}  # file_path -> [CodeEntity]
        self.query_history = deque(maxlen=50)
        
        # Background initialization
        threading.Thread(target=self._initialize, daemon=True).start()
    
    def _initialize(self):
        """Initialize BrainV2 components"""
        global lancedb, Observer, FileSystemEventHandler, OpenAI, TextEmbedding, SentenceTransformer
        
        try:
            if FileSystemEventHandler is object:
                logger.warning("BrainV2 running in lobotomized mode (missing dependencies)")
                return

            logger.info("🧠 BrainV2 waking up...")
            
            # Setup DB
            DB_PATH = Path(__file__).parent.parent.parent / "data" / "lancedb"
            DB_PATH.parent.mkdir(parents=True, exist_ok=True)
            self.db = lancedb.connect(str(DB_PATH))
            
            # Define Enhanced Schema
            schema = pa.schema([
                pa.field("vector", pa.list_(pa.float32(), 384)),
                pa.field("entity_id", pa.string()),
                pa.field("type", pa.string()),
                pa.field("name", pa.string()),
                pa.field("content", pa.string()),
                pa.field("docstring", pa.string()),
                pa.field("signature", pa.string()),
                pa.field("file_path", pa.string()),
                pa.field("start_line", pa.int32()),
                pa.field("end_line", pa.int32()),
                pa.field("imports", pa.list_(pa.string())),
                pa.field("exports", pa.list_(pa.string())),
                pa.field("complexity", pa.int32()),
                pa.field("metadata", pa.string()),  # JSON string
                pa.field("timestamp", pa.float64())
            ])
            
            # Open or Create Codebase Table
            try:
                self.table = self.db.open_table("codebase_v2")
            except:
                self.table = self.db.create_table("codebase_v2", schema=schema)
            
            # [HIPPOCAMPUS] - Memory Table
            memory_schema = pa.schema([
                pa.field("vector", pa.list_(pa.float32(), 384)),
                pa.field("content", pa.string()),
                pa.field("type", pa.string()), # 'fact', 'decision', 'insight', 'todo'
                pa.field("context", pa.string()), # file or related entity
                pa.field("timestamp", pa.float64())
            ])
            
            try:
                self.memory_table = self.db.open_table("memories_v1")
            except:
                self.memory_table = self.db.create_table("memories_v1", schema=memory_schema)
                
            # Load Model
            logger.info("Loading embedding model...")
            # Load Model (Optimized for ONNX)
            logger.info("Loading embedding model...")
            
            self.model = None
            self.use_fastembed = False
            
            # 1. Try FastEmbed (ONNX)
            if HAS_FASTEMBED:
                try:
                    logger.info("🚀 Initializing FastEmbed (ONNX)...")
                    # "BAAI/bge-small-en-v1.5" is a great default for code/text, or "all-MiniLM-L6-v2"
                    # FastEmbed defaults to "BAAI/bge-small-en-v1.5" which is better than MiniLM
                    self.model = TextEmbedding(model_name="BAAI/bge-small-en-v1.5")
                    self.use_fastembed = True
                    logger.info("✅ Embeddings: FastEmbed ONNX Active (High Speed)")
                except Exception as e:
                    logger.error(f"FastEmbed init failed: {e}")
            
            # 2. Fallback to SentenceTransformers
            if not self.model:
                try:
                    self.model = SentenceTransformer("all-MiniLM-L6-v2", device="cpu")
                    try:
                        self.model.to("cuda")
                        logger.info("✅ Embeddings: PyTorch CUDA Active")
                    except:
                        logger.info("⚠️ Embeddings: PyTorch CPU (Slow - Install fastembed for speed)")
                except Exception as e:
                    logger.error(f"Fallback Model Failed: {e}")


            
            # Tier 1: Privacy Gemini (Direct)
            privacy_key = os.getenv("PRIVACY_GEMINI_KEY")
            if privacy_key:
                try:
                    # New google.genai API
                    self.gemini_client = genai.Client(api_key=privacy_key)
                    model_name = os.getenv("PRIVACY_MODEL", "gemini-1.5-flash")
                    self.gemini_model_name = model_name
                    logger.info(f"✅ Gemini Client configured ({model_name})")
                except Exception as e:
                    logger.error(f"Gemini init failed: {e}")
                    self.gemini_client = None
            
            # Tier 2 & 3: OpenRouter
            openrouter_key = os.getenv("OPENROUTER_API_KEY")
            if openrouter_key:
                self.openai_client = OpenAI(
                    base_url="https://openrouter.ai/api/v1",
                    api_key=openrouter_key
                )
            else:
                logger.warning("OPENROUTER_API_KEY not found, falling back to local Ollama")
                self.openai_client = OpenAI(
                    base_url="http://localhost:11434/v1",
                    api_key="ollama"
                )
            
            self.ready = True
            logger.info("🧠 BrainV2 is ONLINE and READY.")
            
            # Start Watchdog
            self._start_watchdog()
            
        except Exception as e:
            logger.error(f"BrainV2 initialization failed: {e}")
            import traceback
            traceback.print_exc()
    
    def _start_watchdog(self):
        if not self.ready: return
        
        project_root = Path(__file__).parent.parent.parent.parent
        event_handler = BrainV2EventHandler(self)
        self.observer = Observer()
        self.observer.schedule(event_handler, str(project_root), recursive=True)
        self.observer.start()
        logger.info(f"🧠 Live Cortex v2 active on {project_root}")
    
    def embed(self, text: str) -> List[float]:
        if not self.ready: return [0.0] * 384
        
        try:
            if self.use_fastembed and self.model:
                # FastEmbed returns a generator of numpy arrays
                embeddings = list(self.model.embed([text]))
                return embeddings[0].tolist()
            
            # Standard SentenceTransformers
            if self.model:
                return self.model.encode(text).tolist()
                
        except Exception as e:
            logger.error(f"Embedding failed: {e}")
            
        return [0.0] * 384

    # --- HIPPOCAMPUS (Memory) ---

    def remember(self, content: str, kind: str = "insight", context: str = "") -> str:
        """Store a memory in the Hippocampus"""
        if not self.ready: return "Brain not ready"
        
        vector = self.embed(f"{kind}: {content} {context}")
        
        self.memory_table.add([{
            "vector": vector,
            "content": content,
            "type": kind,
            "context": context,
            "timestamp": time.time()
        }])
        
        logger.info(f"🧠 Hippocampus stored: [{kind}] {content[:50]}...")
        return f"Memory stored: {content[:50]}..."

    def recall(self, query: str, limit: int = 3) -> List[Dict]:
        """Recall memories related to query"""
        if not self.ready: return []
        
        vector = self.embed(query)
        results = self.memory_table.search(vector).limit(limit).to_list()
        
        return [{
            "content": r["content"],
            "type": r["type"],
            "context": r["context"],
            "score": r.get("_distance", 0),
            "timestamp": r["timestamp"]
        } for r in results]
    
    def index_file(self, path: str):
        """Enhanced indexing with AST parsing & Evolution Tracking"""
        if not self.ready: return
        
        p = Path(path)
        if p.suffix not in ASTCodeParser.EXTENSION_PARSERS and p.suffix not in ['.rs', '.ts', '.tsx', '.js']:
            return
        
        try:
            content = p.read_text(encoding='utf-8', errors='ignore')
            
            # Parse into semantic entities
            new_entities = self.ast_parser.parse_file(str(p), content)
            
            # [EVOLUTION] - Compare with cache for changes
            old_entities = self.entity_cache.get(str(p), [])
            if old_entities:
                self._track_evolution(str(p), old_entities, new_entities)
            
            # Remove old entries for this file
            self.table.delete(f"file_path = '{str(p)}'")
            
            # Index each entity
            chunks = []
            for entity in new_entities:
                # Create semantic text for embedding
                semantic_text = f"{entity.type} {entity.name}"
                if entity.docstring:
                    semantic_text += f" - {entity.docstring}"
                if entity.signature:
                    semantic_text += f" | {entity.signature}"
                semantic_text += f" | {entity.content[:200]}"
                
                vector = self.embed(semantic_text)
                entity.vector = vector
                
                # Add to graph
                self.code_graph.add_entity(entity)
                
                # Store in cache
                self.entity_cache[str(p)] = new_entities  # Update cache
                
                chunks.append({
                    "vector": vector,
                    "entity_id": entity.id,
                    "type": entity.type,
                    "name": entity.name,
                    "content": entity.content,
                    "docstring": entity.docstring or "",
                    "signature": entity.signature or "",
                    "file_path": str(p),
                    "start_line": entity.start_line,
                    "end_line": entity.end_line,
                    "imports": entity.imports,
                    "exports": entity.exports,
                    "complexity": entity.complexity,
                    "metadata": json.dumps(entity.metadata or {}),
                    "timestamp": time.time()
                })
            
            if chunks:
                self.table.add(chunks)
                logger.info(f"🧠 Learned {p.name} - {len(new_entities)} semantic entities")
                
        except Exception as e:
            logger.error(f"Failed to index {path}: {e}")

    def _track_evolution(self, file_path: str, old_entities: List[CodeEntity], new_entities: List[CodeEntity]):
        """Detect semantic changes and store evolution memories"""
        old_map = {e.name: e for e in old_entities if e.type in ('function', 'method', 'class')}
        new_map = {e.name: e for e in new_entities if e.type in ('function', 'method', 'class')}
        
        file_name = Path(file_path).name
        
        # Check for modifications
        for name, new_e in new_map.items():
            if name in old_map:
                old_e = old_map[name]
                # Simple content hash/equality check
                if new_e.content.strip() != old_e.content.strip():
                    # It changed!
                    change_desc = f"Modified {new_e.type} '{name}' in {file_name}"
                    
                    # Detect signature change
                    if new_e.signature != old_e.signature:
                        change_desc += f" (Signature changed: {old_e.signature} -> {new_e.signature})"
                    
                    self.remember(change_desc, kind="evolution", context=file_path)
            else:
                # New entity
                self.remember(f"Created new {new_e.type} '{name}' in {file_name}", kind="evolution", context=file_path)
        
        # Check for deletions
        for name in old_map:
            if name not in new_map:
                self.remember(f"Deleted {old_map[name].type} '{name}' from {file_name}", kind="evolution", context=file_path)
    
    def query(self, text: str, n: int = 5) -> List[SearchResult]:
        """Multi-stage hybrid search"""
        if not self.ready: return []
        
        # Check cache
        cache_key = f"{text}:{n}"
        cached = self.smart_cache.get(cache_key)
        if cached:
            return cached
        
        # Stage 1: Intent Recognition
        intent = self.intent_recognizer.recognize(text)
        logger.info(f"Query intent: {intent.intent_type} | Entities: {intent.entities} | Concepts: {intent.concepts}")
        
        # Stage 2: Vector Search
        vector = self.embed(text)
        vector_results = self.table.search(vector).limit(n * 2).to_list()
        
        # Stage 3: Re-ranking with context
        results = []
        for vr in vector_results:
            # Get entity
            entity = self._reconstruct_entity(vr)
            
            # Calculate relevance score
            score = vr.get('_distance', 0)
            
            # Boost based on intent
            if intent.intent_type == 'find_code':
                # Prefer functions/classes
                if entity.type in ['function', 'class', 'method']:
                    score *= 0.8
            elif intent.intent_type == 'explain':
                # Prefer modules and classes with docstrings
                if entity.docstring:
                    score *= 0.7
            
            # Boost based on entity matching
            if intent.entities:
                for ent in intent.entities:
                    if ent.lower() in entity.name.lower():
                        score *= 0.6
                        break
            
            # Boost based on concepts
            if intent.concepts:
                for concept in intent.concepts:
                    if concept.lower() in entity.content.lower():
                        score *= 0.7
                        break
            
            # Get related entities from graph
            related_ids = self.code_graph.get_related(entity.id, depth=1)
            related_names = [self.code_graph.nodes[rid].name for rid in related_ids if rid in self.code_graph.nodes]
            
            # Create result
            result = SearchResult(
                entity=entity,
                score=score,
                reason=self._explain_match(intent, entity),
                related=related_names[:3]
            )
            results.append(result)
        
        # Stage 4: Sort and limit
        results.sort(key=lambda r: r.score)
        final_results = results[:n]
        
        # Cache
        self.smart_cache.put(cache_key, final_results)
        
        # Store query history
        self.query_history.append({
            'text': text,
            'intent': intent.intent_type,
            'timestamp': time.time(),
            'results': [r.entity.id for r in final_results]
        })
        
        return final_results
    
    def _reconstruct_entity(self, db_result: Dict) -> CodeEntity:
        """Reconstruct CodeEntity from DB result"""
        metadata = json.loads(db_result.get('metadata', '{}'))
        return CodeEntity(
            id=db_result['entity_id'],
            type=db_result['type'],
            name=db_result['name'],
            content=db_result['content'],
            docstring=db_result['docstring'],
            signature=db_result['signature'],
            file_path=db_result['file_path'],
            start_line=db_result['start_line'],
            end_line=db_result['end_line'],
            imports=db_result['imports'],
            exports=db_result['exports'],
            dependencies=[],
            complexity=db_result['complexity'],
            vector=None,
            metadata=metadata
        )
    
    def _explain_match(self, intent: QueryIntent, entity: CodeEntity) -> str:
        """Explain why this entity matches"""
        reasons = []
        
        if intent.intent_type == 'find_code':
            reasons.append(f"Matches {entity.type} '{entity.name}'")
        elif intent.intent_type == 'explain':
            if entity.docstring:
                reasons.append("Has documentation")
            reasons.append(f"Describes {entity.type}")
        
        if intent.entities and any(e.lower() in entity.name.lower() for e in intent.entities):
            reasons.append("Name matches query")
        
        if intent.concepts:
            for concept in intent.concepts:
                if concept.lower() in entity.content.lower():
                    reasons.append(f"Contains {concept}")
                    break
        
        return ", ".join(reasons) if reasons else "Semantic similarity"
    
    def get_stats(self) -> Dict:
        """Get brain statistics"""
        if not self.ready:
            return {"ready": False}
        
        return {
            "ready": True,
            "entities_indexed": len(self.code_graph.nodes),
            "cache_size": len(self.smart_cache.cache),
            "query_history_size": len(self.query_history),
            "graph_nodes": len(self.code_graph.nodes),
            "graph_edges": sum(len(v) for v in self.code_graph.edges.values()),
            "recent_queries": list(self.query_history)[-5:] if self.query_history else []
        }

# --- EVENT HANDLER ---

class BrainV2EventHandler(FileSystemEventHandler):
    def __init__(self, brain: BrainV2):
        self.brain = brain
        self.last_event = 0
        self.lock = threading.Lock()
        self.debounce_seconds = 1.5

    def on_modified(self, event):
        if event.is_directory:
            return
        
        now = time.time()
        with self.lock:
            if now - self.last_event < self.debounce_seconds:
                return
            self.last_event = now
        
        path = Path(event.src_path)
        
        # Ignore patterns - K_OS Brain Barrier
        ignore_dirs = {
            # Build artifacts
            "target", "node_modules", "dist", "build", "gen", "coverage",
            # Version control & IDE
            ".git", ".vscode", ".idea", ".cargo", ".cursor-rust-tools",
            # Python noise
            ".venv", "__pycache__", "src-python", "data",
            # Agent/AI folders
            ".openmcp", ".gemini", ".claude", ".agent",
            # Non-code folders
            "imports", "IMPORTS", "assets", "public", "docs", "icons", "resources", "capabilities",
            # Utility/legacy
            "backups", "logs", "tmp", "tools", "mcp-server", "_legacy"
        }
        if any(d in path.parts for d in ignore_dirs):
            return
        
        if path.suffix in ASTCodeParser.EXTENSION_PARSERS or path.suffix in ['.rs', '.ts', '.tsx', '.js']:
            # Silent indexing - no spam
            self.brain.index_file(str(path))
            
            # Also reindex files that import this one
            self._reindex_importers(path)

    def on_deleted(self, event):
        """Handle file deletion"""
        if event.is_directory:
            return
            
        path = Path(event.src_path)
        
        # Only process actual code files, not temp files
        if path.suffix not in ['.py', '.rs', '.ts', '.tsx', '.js']:
            return
        
        # Silent cleanup - no spam
        try:
            self.brain.table.delete(f"file_path = '{str(path)}'")
            # Remove from cache
            if str(path) in self.brain.entity_cache:
                del self.brain.entity_cache[str(path)]
        except Exception as e:
            pass  # Silent fail for cleanup

    def _reindex_importers(self, changed_file: Path):
        """Reindex files that import the changed file"""
        file_stem = changed_file.stem
        
        # Find files that import this module
        for file_path, entities in self.brain.entity_cache.items():
            for entity in entities:
                if file_stem in entity.imports:
                    logger.info(f"🧠 Reindexing dependent: {Path(file_path).name}")
                    self.brain.index_file(file_path)
                    break

# --- EXPORTED API ---

# Singleton
_BRAIN_V2 = BrainV2()

@register("brain_v2_status", mcp=True)
def brain_v2_status() -> Dict:
    return _BRAIN_V2.get_stats()

@register("brain_v2_query", mcp=True)
def brain_v2_query(prompt: str, limit: int = 5) -> List[Dict]:
    """Enhanced semantic search with intent recognition"""
    if not _BRAIN_V2.ready:
        return [{"error": "Brain initializing"}]
    
    results = _BRAIN_V2.query(prompt, limit)
    
    # Convert to serializable format
    return [{
        "entity_id": r.entity.id,
        "type": r.entity.type,
        "name": r.entity.name,
        "file": r.entity.file_path,
        "line": r.entity.start_line,
        "signature": r.entity.signature,
        "docstring": r.entity.docstring,
        "score": r.score,
        "reason": r.reason,
        "related": r.related,
        "content_preview": r.entity.content[:150].replace('\n', ' ')
    } for r in results]

@register("brain_v2_ask", mcp=True)
def brain_v2_ask(question: str) -> str:
    """Ask with enhanced context"""
    if not _BRAIN_V2.ready:
        return "Brain initializing..."
    
    # Get intent
    intent = _BRAIN_V2.intent_recognizer.recognize(question)
    
    # Get context
    context_hits = _BRAIN_V2.query(question, n=3)
    context_str = "\n\n".join([
        f"--- {r.entity.name} ({r.entity.type}) ---\n{r.entity.content[:300]}"
        for r in context_hits
    ])
    
    # [HIPPOCAMPUS] - Inject Memories
    memories = _BRAIN_V2.recall(question, limit=2)
    if memories:
        memory_str = "\n".join([f"MEMORY ({m['type']}): {m['content']}" for m in memories])
        context_str += f"\n\n--- RELEVANT MEMORIES ---\n{memory_str}"
    
    # Enhanced prompt
    prompt = f"""
You are K_OS Data Keeper v2. Answer based on codebase context and user intent.
    
INTENT: {intent.intent_type}
CONCEPTS: {', '.join(intent.concepts)}
CONTEXT: {intent.context}

CODEBASE CONTEXT:
{context_str}

QUESTION: {question}

Answer concisely and reference specific code entities when relevant.
"""
    
    # Privacy Check
    privacy_mode = os.getenv("PRIVACY_MODE", "false").lower() == "true"
    privacy_guard = None
    
    # Initialize guard once if needed
    if privacy_mode and _BRAIN_V2.ready:
        sensitive_terms = {node.name for node in _BRAIN_V2.code_graph.nodes.values()}
        privacy_guard = PrivacyGuard(sensitive_terms)

    # Define the Tiers
    tiers = []
    
    # Tier 1: Privacy Gemini
    if _BRAIN_V2.gemini_client:
        tiers.append({
            "type": "google",
            "client": _BRAIN_V2.gemini_client,
            "model": _BRAIN_V2.gemini_model_name, 
            "name": "Privacy-Gemini"
        })
        
    # Tier 2: OpenRouter (GPT-OSS 120B)
    if _BRAIN_V2.openai_client:
        tiers.append({
            "type": "openai",
            "client": _BRAIN_V2.openai_client,
            "model": os.getenv("BRAIN_MODEL", "openai/gpt-oss-120b:exacto"),
            "name": "OpenRouter-GPT-OSS"
        })

    last_error = None
    for tier in tiers:
        try:
            current_prompt = prompt
            is_sanitized = False
            
            # SELECTIVE CLOAKING:
            # Only sanitize if privacy is ON and the tier is NOT the trusted Privacy-Gemini
            if privacy_guard and tier['name'] != "Privacy-Gemini":
                logger.info(f"🛡️ Cloaking prompt for public tier: {tier['name']}")
                current_prompt = privacy_guard.sanitize(prompt)
                is_sanitized = True
            
            logger.info(f"🔮 Oracle consulting {tier['name']}...")
            
            response_text = ""
            if tier['type'] == 'google':
                # New google.genai Client API
                response = tier['client'].models.generate_content(
                    model=tier['model'],
                    contents=current_prompt
                )
                response_text = response.text
            else:
                # OpenAI/OpenRouter Call
                response = tier['client'].chat.completions.create(
                    model=tier['model'],
                    messages=[{"role": "user", "content": current_prompt}],
                    extra_headers={
                        "HTTP-Referer": "https://k-os.app",
                        "X-Title": "K_OS Brain v2",
                    } if "openrouter" in tier['name'].lower() else {},
                    timeout=45.0
                )
                response_text = response.choices[0].message.content
            
            # De-obfuscate ONLY if we sanitized this specific request
            if is_sanitized:
                response_text = privacy_guard.desanitize(response_text)
                
            return response_text
        except Exception as e:
            last_error = e
            logger.warning(f"⚠️ {tier['name']} failed: {e}. Escalating to next tier...")
            continue
            
    return f"Oracle collapsed. All tiers failed. Last error: {last_error}"

@register("brain_v2_index_dir", mcp=True)
def brain_v2_index_dir(path: str) -> str:
    """Enhanced directory indexing"""
    if not _BRAIN_V2.ready:
        return "Brain initializing"
    
    count = 0
    p = Path(path)
    if not p.is_absolute():
        p = (Path(__file__).parent.parent.parent.parent / path).resolve()
    
    # K_OS "Brain Barrier" - Ignored Directories
    ignored_dirs = {
        # Build artifacts
        "target", "node_modules", "dist", "build", "gen", "coverage",
        # Version control & IDE
        ".git", ".vscode", ".idea", ".cargo", ".cursor-rust-tools",
        # Python noise
        ".venv", "__pycache__", "src-python", "data",
        # Agent/AI folders
        ".openmcp", ".gemini", ".claude", ".agent",
        # Non-code folders
        "imports", "IMPORTS", "assets", "public", "docs", "icons", "resources", "capabilities",
        # Utility/legacy
        "backups", "logs", "tmp", "tools", "mcp-server", "_legacy"
    }
    
    for root, dirs, files in os.walk(p):
        # In-place filtering to prevent walking into blacklisted dirs
        dirs[:] = [d for d in dirs if d not in ignored_dirs and not d.startswith('.')]
        
        for file in files:
            file_path = Path(root) / file
            if file_path.suffix in ASTCodeParser.EXTENSION_PARSERS or file_path.suffix in ['.rs', '.ts', '.tsx', '.js']:
                _BRAIN_V2.index_file(str(file_path))
                count += 1
    
    return f"🧠 BrainV2 indexed {count} files in {path}"

@register("brain_v2_purge", mcp=True)
def brain_v2_purge(pattern: str) -> str:
    """Purge entries matching a pattern from the index"""
    if not _BRAIN_V2.ready:
        return "Brain initializing"
    
    try:
        # Count matching entries using native LanceDB
        all_entries = _BRAIN_V2.table.to_arrow()
        file_paths = all_entries.column("file_path").to_pylist()
        count = sum(1 for fp in file_paths if pattern.lower() in fp.lower())
        
        if count == 0:
            return f"No entries found matching '{pattern}'"
        
        # Delete from DB using SQL-like filter
        _BRAIN_V2.table.delete(f"file_path LIKE '%{pattern}%'")
        
        # Clean entity cache
        keys_to_remove = [k for k in _BRAIN_V2.entity_cache if pattern.lower() in k.lower()]
        for k in keys_to_remove:
            del _BRAIN_V2.entity_cache[k]
        
        # Clear smart cache (queries might now return different results)
        _BRAIN_V2.smart_cache.cache.clear()
        _BRAIN_V2.smart_cache.access_order.clear()
        
        return f"🗑️ Purged {count} entries matching '{pattern}'"
        
    except Exception as e:
        return f"Purge failed: {e}"

@register("brain_v2_graph", mcp=True)
def brain_v2_graph(entity_name: str) -> Dict:
    """Get dependency graph for entity"""
    if not _BRAIN_V2.ready:
        return {"error": "Brain initializing"}
    
    entity_ids = _BRAIN_V2.code_graph.find_by_name(entity_name)
    if not entity_ids:
        return {"error": "Entity not found"}
    
    entity_id = entity_ids[0]
    related = _BRAIN_V2.code_graph.get_related(entity_id, depth=2)
    callers = _BRAIN_V2.code_graph.get_callers(entity_id)
    
    return {
        "entity": asdict(_BRAIN_V2.code_graph.nodes[entity_id]),
        "related": [_BRAIN_V2.code_graph.nodes[rid].name for rid in related if rid in _BRAIN_V2.code_graph.nodes],
        "callers": [_BRAIN_V2.code_graph.nodes[cid].name for cid in callers if cid in _BRAIN_V2.code_graph.nodes]
    }


@register("brain_v2_remember", mcp=True)
def brain_v2_remember(content: str, kind: str = "insight", context: str = "") -> str:
    """Store a permanent memory (insight, decision, fact)"""
    return _BRAIN_V2.remember(content, kind, context)

@register("brain_v2_recall", mcp=True)
def brain_v2_recall(query: str) -> List[Dict]:
    """Search stored memories"""
    return _BRAIN_V2.recall(query)


@register("brain_v2_build_doctor", mcp=True)
def brain_v2_build_doctor(build_output: str) -> str:
    """
    Analyzes compiler/build errors by cross-referencing with the codebase index.
    Consults the Triple-Tier Oracle Council for a definitive fix.
    """
    if not _BRAIN_V2.ready:
        return "Brain initializing..."
        
    logger.info("🛠️ Build Doctor examining patient (build output)...")
    
    # Common compiler error patterns (Rust, TS, etc.)
    # Rust: --> path:line:col
    # TS/Vite: path:line:col - error ...
    path_patterns = [
        r"--> (.*?):(\d+):(\d+)",          # Rust
        r"([a-zA-Z]:[\\/].*?):(\d+):(\d+)", # Win absolute paths
        r"([./\\].*?):(\d+):(\d+)",         # Relative paths
        r'File "(.*?)", line (\d+)()'       # Python traceback (3rd group for col compatibility)
    ]
    
    context_reports = []
    seen_files = set()
    evolution_context = []

    for pattern in path_patterns:
        matches = re.findall(pattern, build_output)
        for file_path, line_str, col_str in matches:
            # Clean up paths (Windows is messy)
            clean_path = file_path.strip().replace('\\\\', '/').replace('\\', '/')
            if clean_path in seen_files or "node_modules" in clean_path or "target" in clean_path:
                continue
            seen_files.add(clean_path)
            
            # [EVOLUTION] - Check if this file changed recently
            file_memories = _BRAIN_V2.recall(clean_path, limit=5)
            for m in file_memories:
                # Normalize memory context for comparison
                mem_context = str(m.get('context', '')).replace('\\', '/')
                if m['type'] == 'evolution' and clean_path in mem_context:
                    evolution_context.append(f"- {time.ctime(m['timestamp'])}: {m['content']}")

            try:
                line_num = int(line_str)
                # Try to resolve relative to project root
                p = Path(clean_path)
                if not p.is_absolute():
                    p = (root_dir / clean_path).resolve()
                
                if p.exists():
                    with open(p, 'r', encoding='utf-8', errors='ignore') as f:
                        lines = f.readlines()
                        start = max(0, line_num - 20)
                        end = min(len(lines), line_num + 20)
                        code_context = "".join([f"{i+1:4} | {lines[i]}" for i in range(start, end)])
                        
                    context_reports.append(f"### File: {clean_path} (Line {line_num})\n```\n{code_context}\n```")
            except Exception as e:
                logger.error(f"Error gathering context for {clean_path}: {e}")

    # Inject evolution context if any found
    evolution_str = ""
    if evolution_context:
        evolution_str = "\n\nRECENT CHANGES (Potential Regressions):\n" + "\n".join(set(evolution_context))

    if not context_reports:
        # Fallback to direct semantic search if no paths found
        logger.warning("No file paths found in build output, attempting semantic diagnosis...")
        return brain_v2_ask(f"Diagnose this build failure and suggest a fix:\n{build_output}")

    # Prepare prompt for the Council
    diagnosis_prompt = f"""
SYSTEM: You are the K_OS Build Doctor. 
A build has failed. Analyze the compiler output and the provided code context to suggest a definitive fix.

ERRORS:
{build_output}
{evolution_str}

CODE CONTEXT FROM FILES:
{"".join(context_reports)}

DIAGNOSIS PROTOCOL:
1. Identify the EXACT line causing the error.
2. Check if this is a regression (see RECENT CHANGES above).
3. Provide the full code block for the fix.
4. If multiple files are involved, list them all.
"""

    return brain_v2_ask(diagnosis_prompt)


def gatekeeper_evaluate(command: str) -> Dict[str, Any]:
    if not _BRAIN_V2.ready or _BRAIN_V2.openai_client is None:
        return {
            "decision": "allow",
            "reason": "Brain not ready, allowing by default",
            "safe_command": command,
        }
    context_hits = _BRAIN_V2.query(command, n=3)
    context_parts = []
    for r in context_hits:
        name = r.entity.name
        etype = r.entity.type
        path = r.entity.file_path
        line = r.entity.start_line
        snippet = r.entity.content[:200].replace("\n", " ")
        context_parts.append(f"{name} ({etype}) @ {path}:{line} -> {snippet}")
    context_str = "\n\n".join(context_parts) if context_parts else "No relevant code context."
    prompt = f"""
You are the K_OS Terminal Gatekeeper.

K_OS CONTEXT:
{context_str}

User is about to run this shell command:
{command}

Decide whether this command should run.
Return a JSON object with these fields:
- decision: one of "allow", "deny", "modify"
- reason: short explanation
- safe_command: command to run if decision is "allow" or "modify"

Respond with JSON only.
"""
    try:
        response = _BRAIN_V2.openai_client.chat.completions.create(
            model="llama3",
            messages=[{"role": "user", "content": prompt}],
            temperature=0,
        )
        content = response.choices[0].message.content.strip()
        data = json.loads(content)
        decision = str(data.get("decision", "allow")).lower()
        if decision not in {"allow", "deny", "modify"}:
            decision = "allow"
        safe_command = data.get("safe_command") or command
        reason = data.get("reason") or ""
        return {
            "decision": decision,
            "reason": reason,
            "safe_command": safe_command,
        }
    except Exception as e:
        logger.error(f"Gatekeeper error: {e}")
        return {
            "decision": "allow",
            "reason": f"Gatekeeper error, allowing by default: {e}",
            "safe_command": command,
        }


@register("brain_v2_gatekeep_command", mcp=True)
def brain_v2_gatekeep_command(command: str) -> Dict[str, Any]:
    return gatekeeper_evaluate(command)

def repl():
    """Interactive BrainV2 Console"""
    print("\n🧠 K_OS Brain v2 - Multi-Modal Semantic Understanding")
    print("=" * 60)
    print("Commands:")
    print("  <query> - Semantic search with intent recognition")
    print("  /ask <question> - Ask with context")
    print("  /graph <entity> - Show dependency graph")
    print("  /stats - Show brain statistics")
    print("  /index <path> - Index directory")
    print("  /q - Quit")
    print()
    
    # Wait for ready
    if not _BRAIN_V2.ready:
        print("Initializing Cortex...", end="", flush=True)
        while not _BRAIN_V2.ready:
            time.sleep(0.5)
            print(".", end="", flush=True)
        print(" OPERATIONAL.\n")

    while True:
        try:
            user_input = input("BrainV2> ").strip()
            if not user_input:
                continue
            
            if user_input.lower() in ('/q', '/quit', 'exit'):
                break
            
            if user_input.startswith('/ask '):
                question = user_input[5:].strip()
                print(f"\n{brain_v2_ask(question)}\n")
                continue
            
            if user_input.startswith('/graph '):
                entity = user_input[7:].strip()
                result = brain_v2_graph(entity)
                print(json.dumps(result, indent=2))
                continue
            
            if user_input.startswith('/stats'):
                stats = _BRAIN_V2.get_stats()
                print(json.dumps(stats, indent=2))
                continue
            
            if user_input.startswith('/index '):
                path = user_input[7:].strip()
                print(brain_v2_index_dir(path))
                continue
            
            if user_input.startswith('/purge '):
                pattern = user_input[7:].strip()
                print(brain_v2_purge(pattern))
                continue
            
            # Default: Enhanced query
            start = time.time()
            results = _BRAIN_V2.query(user_input, n=3)
            elapsed = time.time() - start
            
            print(f"\nFound {len(results)} matches in {elapsed:.3f}s:")
            for i, r in enumerate(results):
                print(f"\n--- [{i+1}] {r.entity.name} ({r.entity.type}) ---")
                print(f"    Score: {r.score:.3f} | Reason: {r.reason}")
                print(f"    File: {r.entity.file_path}:{r.entity.start_line}")
                if r.entity.signature:
                    print(f"    Signature: {r.entity.signature}")
                if r.related:
                    print(f"    Related: {', '.join(r.related)}")
                snippet = r.entity.content.replace('\n', ' ')[:150]
                print(f"    Preview: {snippet}...")
                
        except KeyboardInterrupt:
            print("\n")
            break
        except Exception as e:
            print(f"Error: {e}")

def gatekeeper_repl():
    print("\n🛡️ K_OS Brain v2 Gatekeeper")
    print("=" * 60)
    print("Type shell commands to be reviewed before execution.")
    print("Commands:")
    print("  <command>        - Propose a shell command")
    print("  /q, /quit, exit  - Quit gatekeeper shell")
    print()
    if not _BRAIN_V2.ready:
        print("Initializing Cortex...", end="", flush=True)
        while not _BRAIN_V2.ready:
            time.sleep(0.5)
            print(".", end="", flush=True)
        print(" OPERATIONAL.\n")
    while True:
        try:
            user_input = input("Gatekeeper> ").strip()
            if not user_input:
                continue
            if user_input.lower() in ("/q", "/quit", "exit"):
                break
            result = gatekeeper_evaluate(user_input)
            decision = result.get("decision", "allow")
            reason = result.get("reason", "")
            safe_command = result.get("safe_command") or user_input
            print(f"\nDecision: {decision.upper()}")
            if reason:
                print(f"Reason: {reason}")
            if decision == "deny":
                print("Command blocked.\n")
                continue
            if decision == "modify" and safe_command != user_input:
                print(f"Proposed command: {safe_command}")
                confirm = input("Run modified command? [y/N] ").strip().lower()
                if confirm != "y":
                    print("Command cancelled.\n")
                    continue
            else:
                print(f"Running: {safe_command}")
            try:
                subprocess.run(safe_command, shell=True)
            except Exception as e:
                print(f"Execution error: {e}")
            print()
        except KeyboardInterrupt:
            print("\n")
            break
        except Exception as e:
            print(f"Error: {e}")

if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "gatekeeper":
        gatekeeper_repl()
    else:
        repl()

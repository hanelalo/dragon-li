import os
import sqlite3
import logging
import json
from datetime import datetime, timezone
import frontmatter
from yaml.error import YAMLError

logger = logging.getLogger("uvicorn.error")

class SkillManager:
    _instance = None

    def __new__(cls):
        if cls._instance is None:
            cls._instance = super(SkillManager, cls).__new__(cls)
            cls._instance._initialized = False
        return cls._instance

    def __init__(self):
        if self._initialized:
            return
        
        # Determine skills_dir based on DRAGON_LI_DB_PATH if available
        db_path = os.environ.get("DRAGON_LI_DB_PATH")
        if db_path:
            # e.g. ~/.dragon-li/data/dragon_li.db -> ~/.dragon-li/skills
            runtime_root = os.path.dirname(os.path.dirname(db_path))
            self.skills_dir = os.path.join(runtime_root, "skills")
        else:
            self.skills_dir = os.path.expanduser("~/.dragon-li/skills")
            
        self._initialized = True

    def get_db_connection(self):
        db_path = os.environ.get("DRAGON_LI_DB_PATH")
        if not db_path or not os.path.exists(db_path):
            return None
        conn = sqlite3.connect(db_path)
        conn.row_factory = sqlite3.Row
        return conn

    def parse_skill_md(self, filepath):
        """
        Parses a SKILL.md file.
        Returns a tuple of (frontmatter_dict, markdown_body).
        Raises Exception if invalid or missing required fields.
        """
        try:
            with open(filepath, 'r', encoding='utf-8-sig') as f:
                post = frontmatter.load(f)
        except YAMLError as e:
            raise ValueError(f"Invalid YAML frontmatter: {e}")
        except Exception as e:
            raise ValueError(f"Failed to parse file: {e}")

        # python-frontmatter treats empty files as having no metadata
        # We enforce that the metadata dictionary is present and not empty
        if not post.metadata:
            raise ValueError("Missing or empty YAML frontmatter")

        frontmatter_dict = post.metadata
        markdown_body = post.content.strip()

        if 'name' not in frontmatter_dict:
            raise ValueError("Missing 'name' in frontmatter")
        
        if 'description' not in frontmatter_dict:
            raise ValueError("Missing 'description' in frontmatter")

        return frontmatter_dict, markdown_body

    def scan_skills_directory(self):
        """
        Scans skills directory and upserts discovered skills into the capabilities table.
        """
        conn = self.get_db_connection()
        if not conn:
            logger.warning("DRAGON_LI_DB_PATH not set or invalid, skipping skill sync to DB.")
            return

        now_iso = datetime.now(timezone.utc).isoformat()

        try:
            cursor = conn.cursor()
            
            # Find existing skills to handle soft deletes or updates
            # Fetch ALL skills regardless of deleted_at to properly handle restoration
            cursor.execute("SELECT id, name, deleted_at FROM capabilities WHERE type = 'skill'")
            existing_skills = {row["name"]: {"id": row["id"], "deleted": row["deleted_at"] is not None} for row in cursor.fetchall()}
            
            found_skill_names = set()

            if os.path.exists(self.skills_dir):
                with os.scandir(self.skills_dir) as entries:
                    for entry in entries:
                        if entry.is_dir():
                            skill_md_path = os.path.join(entry.path, "SKILL.md")
                            if os.path.exists(skill_md_path):
                                try:
                                    frontmatter_dict, _ = self.parse_skill_md(skill_md_path)
                                    skill_name = frontmatter_dict["name"]
                                    skill_desc = frontmatter_dict["description"]
                                    
                                    # Validate directory name matches skill name
                                    if skill_name != entry.name:
                                        logger.warning(f"Skill name '{skill_name}' in {skill_md_path} does not match directory name '{entry.name}'. Skipping.")
                                        continue
                                    
                                    found_skill_names.add(skill_name)
                                    
                                    input_schema = json.dumps({
                                        "type": "object",
                                        "properties": {
                                            "task_context": {
                                                "type": "string",
                                                "description": "Context and instructions for the skill to execute."
                                            }
                                        },
                                        "required": ["task_context"]
                                    })

                                    if skill_name in existing_skills:
                                        # Update existing, potentially restoring it if it was soft-deleted
                                        skill_info = existing_skills[skill_name]
                                        skill_id = skill_info["id"]
                                        cursor.execute("""
                                            UPDATE capabilities 
                                            SET description = ?, input_schema_json = ?, updated_at = ?, deleted_at = NULL
                                            WHERE id = ?
                                        """, (skill_desc, input_schema, now_iso, skill_id))
                                        if skill_info["deleted"]:
                                            logger.info(f"Restored soft-deleted skill: {skill_name}")
                                    else:
                                        # Insert new
                                        import uuid
                                        skill_id = str(uuid.uuid4())
                                        cursor.execute("""
                                            INSERT INTO capabilities (id, type, name, description, input_schema_json, risk_level, enabled, created_at, updated_at)
                                            VALUES (?, 'skill', ?, ?, ?, 'low', 0, ?, ?)
                                        """, (skill_id, skill_name, skill_desc, input_schema, now_iso, now_iso))
                                    
                                    logger.info(f"Successfully synced skill: {skill_name}")
                                except Exception as e:
                                    logger.error(f"Failed to parse skill at {skill_md_path}: {e}")
            else:
                logger.info(f"Skills directory {self.skills_dir} does not exist. All existing skills will be marked as deleted.")

            # Mark missing skills as deleted
            for existing_name, skill_info in existing_skills.items():
                if existing_name not in found_skill_names and not skill_info["deleted"]:
                    cursor.execute("""
                        UPDATE capabilities
                        SET deleted_at = ?, updated_at = ?
                        WHERE id = ?
                    """, (now_iso, now_iso, skill_info["id"]))
                    logger.info(f"Marked skill {existing_name} as deleted")

            conn.commit()
        except sqlite3.Error as e:
            logger.error(f"Database error during skill scan: {e}")
            conn.rollback()
        finally:
            conn.close()

    def get_delegate_tool_schema(self):
        """
        Dynamically builds the tool schema for 'delegate_to_skill'.
        Returns None if no skills are active.
        """
        conn = self.get_db_connection()
        if not conn:
            return None
        
        try:
            cursor = conn.cursor()
            cursor.execute("SELECT id, name, description FROM capabilities WHERE type = 'skill' AND enabled = 1 AND deleted_at IS NULL")
            rows = cursor.fetchall()
            if not rows:
                return None
            
            skill_names = []
            skill_descriptions = []
            
            for row in rows:
                name = row["name"]
                desc = row["description"]
                skill_names.append(name)
                skill_descriptions.append(f"- **{name}**: {desc}")
                
            desc_text = "Delegate the task to a specialized skill expert if the user's request matches one of the following domains:\n" + "\n".join(skill_descriptions)
            
            return {
                "type": "function",
                "function": {
                    "name": "delegate_to_skill",
                    "description": desc_text,
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "skill_name": {
                                "type": "string",
                                "enum": skill_names,
                                "description": "The exact name of the skill to delegate to."
                            },
                            "task_context": {
                                "type": "string",
                                "description": "A refined, detailed prompt or context extracted from the user's request, instructing the skill expert what needs to be done."
                            }
                        },
                        "required": ["skill_name", "task_context"]
                    }
                }
            }
        except Exception as e:
            logger.error(f"Failed to build delegate tool schema: {e}")
            return None
        finally:
            conn.close()

    def get_allowed_tools(self, skill_id: str) -> list:
        conn = self.get_db_connection()
        if not conn:
            return []
        try:
            cursor = conn.cursor()
            cursor.execute("SELECT name FROM capabilities WHERE id = ?", (skill_id,))
            row = cursor.fetchone()
            if not row:
                return []
            skill_name = row["name"]
            
            skill_md_path = os.path.join(self.skills_dir, skill_name, "SKILL.md")
            if not os.path.exists(skill_md_path):
                return []
                
            frontmatter_dict, _ = self.parse_skill_md(skill_md_path)
            return frontmatter_dict.get("allowed-tools") or []
        except Exception as e:
            logger.error(f"Failed to get allowed tools for skill: {e}")
            return []
        finally:
            conn.close()

    def get_tools_for_skill(self, skill_id: str) -> list:
        allowed_tools = self.get_allowed_tools(skill_id)
        
        conn = self.get_db_connection()
        if not conn:
            return []
        
        try:
            cursor = conn.cursor()
            cursor.execute("SELECT name FROM capabilities WHERE id = ?", (skill_id,))
            row = cursor.fetchone()
            if not row:
                return []
            skill_name = row["name"]
            
            from agent_mcp.client import mcp_manager
            all_mcp_tools = mcp_manager.get_all_tools()
            
            filtered_tools = []
            for t in all_mcp_tools:
                if t["function"]["name"] in allowed_tools:
                    filtered_tools.append(t)
            
            scripts_dir = os.path.join(self.skills_dir, skill_name, "scripts")
            skill_dir = os.path.join(self.skills_dir, skill_name)
            if os.path.exists(scripts_dir) and os.path.isdir(scripts_dir):
                import subprocess
                for script_file in os.listdir(scripts_dir):
                    script_path = os.path.join(scripts_dir, script_file)
                    if os.path.isfile(script_path) and os.access(script_path, os.X_OK):
                        try:
                            result = subprocess.run([script_path, "--schema"], capture_output=True, text=True, cwd=skill_dir, timeout=5)
                            if result.returncode == 0:
                                schema = json.loads(result.stdout.strip())
                                filtered_tools.append({
                                    "type": "function",
                                    "function": schema
                                })
                        except Exception as e:
                            logger.error(f"Failed to load schema from local tool {script_path}: {e}")
                            
            return filtered_tools
            
        except Exception as e:
            logger.error(f"Failed to get tools for skill: {e}")
            return []
        finally:
            conn.close()

    def execute_local_tool(self, skill_id: str, tool_name: str, arguments: dict) -> str:
        conn = self.get_db_connection()
        if not conn:
            raise Exception("No DB connection")
            
        try:
            cursor = conn.cursor()
            cursor.execute("SELECT name FROM capabilities WHERE id = ?", (skill_id,))
            row = cursor.fetchone()
            if not row:
                raise Exception("Skill not found")
            skill_name = row["name"]
            
            scripts_dir = os.path.join(self.skills_dir, skill_name, "scripts")
            skill_dir = os.path.join(self.skills_dir, skill_name)
            if not os.path.exists(scripts_dir):
                raise Exception("Skill scripts directory not found")
                
            import subprocess
            target_script = None
            for script_file in os.listdir(scripts_dir):
                script_path = os.path.join(scripts_dir, script_file)
                if os.path.isfile(script_path) and os.access(script_path, os.X_OK):
                    try:
                        result = subprocess.run([script_path, "--schema"], capture_output=True, text=True, cwd=skill_dir, timeout=5)
                        if result.returncode == 0:
                            schema = json.loads(result.stdout.strip())
                            if schema.get("name") == tool_name:
                                target_script = script_path
                                break
                    except Exception:
                        continue
                        
            if not target_script:
                raise Exception(f"Local tool {tool_name} not found in skill {skill_name}")
                
            result = subprocess.run(
                [target_script],
                input=json.dumps(arguments),
                capture_output=True,
                text=True,
                cwd=skill_dir,
                timeout=120
            )
            
            if result.returncode != 0:
                return f"Error: {result.stderr}"
            return result.stdout
            
        except Exception as e:
            logger.error(f"Failed to execute local tool {tool_name}: {e}")
            return f"Error: {str(e)}"
        finally:
            conn.close()

skill_manager = SkillManager()

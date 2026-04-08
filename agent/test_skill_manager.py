import os
import unittest
import shutil
import sqlite3
import json
from skill_manager import SkillManager

class TestSkillManager(unittest.TestCase):
    def setUp(self):
        self.manager = SkillManager()
        self.test_dir = "/tmp/test_skills"
        os.makedirs(self.test_dir, exist_ok=True)
        self.manager.skills_dir = self.test_dir
        
        self.db_path = "/tmp/test_dragon_li.db"
        os.environ["DRAGON_LI_DB_PATH"] = self.db_path
        
        # init test db
        conn = sqlite3.connect(self.db_path)
        cursor = conn.cursor()
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS capabilities (
                id TEXT PRIMARY KEY,
                type TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                input_schema_json TEXT,
                risk_level TEXT NOT NULL,
                enabled BOOLEAN NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                deleted_at TEXT
            )
        """)
        conn.commit()
        conn.close()

    def tearDown(self):
        if os.path.exists(self.test_dir):
            shutil.rmtree(self.test_dir)
        if os.path.exists(self.db_path):
            os.remove(self.db_path)

    def test_valid_skill(self):
        skill_path = os.path.join(self.test_dir, "SKILL.md")
        with open(skill_path, "w") as f:
            f.write("---\nname: web-dev\ndescription: A cool web dev skill\n---\n# Markdown Body\nHello World!")
        
        frontmatter, body = self.manager.parse_skill_md(skill_path)
        self.assertEqual(frontmatter["name"], "web-dev")
        self.assertEqual(frontmatter["description"], "A cool web dev skill")
        self.assertIn("Hello World!", body)

    def test_missing_frontmatter(self):
        skill_path = os.path.join(self.test_dir, "SKILL.md")
        with open(skill_path, "w") as f:
            f.write("# Markdown Body\nHello World!")
        
        with self.assertRaises(ValueError) as ctx:
            self.manager.parse_skill_md(skill_path)
        self.assertIn("Missing or empty YAML frontmatter", str(ctx.exception))

    def test_unclosed_frontmatter(self):
        skill_path = os.path.join(self.test_dir, "SKILL.md")
        with open(skill_path, "w") as f:
            f.write("---\nname: test\ndescription: Test\n# Markdown Body\nHello World!")
        
        with self.assertRaises(ValueError) as ctx:
            self.manager.parse_skill_md(skill_path)
        # python-frontmatter parsing doesn't throw a typical "unclosed" error, 
        # it treats the whole file as frontmatter and thus content will be empty
        # or it might throw yaml parser error depending on content
        pass

    def test_missing_required_fields(self):
        skill_path = os.path.join(self.test_dir, "SKILL.md")
        with open(skill_path, "w") as f:
            f.write("---\nname: web-dev\n---\n# Markdown Body\nHello World!")
        
        with self.assertRaises(ValueError) as ctx:
            self.manager.parse_skill_md(skill_path)
        self.assertIn("Missing 'description' in frontmatter", str(ctx.exception))

    def test_scan_skills_directory(self):
        skill_name = "test-skill"
        skill_dir = os.path.join(self.test_dir, skill_name)
        os.makedirs(skill_dir, exist_ok=True)
        skill_path = os.path.join(skill_dir, "SKILL.md")
        
        with open(skill_path, "w") as f:
            f.write(f"---\nname: {skill_name}\ndescription: Test skill description\n---\n# Test Body")
            
        self.manager.scan_skills_directory()
        
        conn = sqlite3.connect(self.db_path)
        cursor = conn.cursor()
        cursor.execute("SELECT name, description, deleted_at FROM capabilities WHERE type = 'skill'")
        results = cursor.fetchall()
        conn.close()
        
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][0], skill_name)
        self.assertEqual(results[0][1], "Test skill description")
        self.assertIsNone(results[0][2])
        
    def test_scan_skills_soft_delete_and_restore(self):
        skill_name = "temp-skill"
        skill_dir = os.path.join(self.test_dir, skill_name)
        os.makedirs(skill_dir, exist_ok=True)
        skill_path = os.path.join(skill_dir, "SKILL.md")
        
        with open(skill_path, "w") as f:
            f.write(f"---\nname: {skill_name}\ndescription: Initial description\n---\n# Test Body")
            
        # Insert
        self.manager.scan_skills_directory()
        
        # Soft delete
        shutil.rmtree(skill_dir)
        self.manager.scan_skills_directory()
        
        conn = sqlite3.connect(self.db_path)
        cursor = conn.cursor()
        cursor.execute("SELECT name, deleted_at FROM capabilities WHERE name = ?", (skill_name,))
        result = cursor.fetchone()
        self.assertIsNotNone(result[1]) # deleted_at is not null
        
        # Restore
        os.makedirs(skill_dir, exist_ok=True)
        with open(skill_path, "w") as f:
            f.write(f"---\nname: {skill_name}\ndescription: Restored description\n---\n# Test Body")
            
        self.manager.scan_skills_directory()
        
        cursor.execute("SELECT name, description, deleted_at FROM capabilities WHERE name = ?", (skill_name,))
        result = cursor.fetchone()
        self.assertEqual(result[1], "Restored description")
        self.assertIsNone(result[2]) # deleted_at is null again
        conn.close()
        
    def test_scan_skills_missing_directory(self):
        skill_name = "test-skill"
        skill_dir = os.path.join(self.test_dir, skill_name)
        os.makedirs(skill_dir, exist_ok=True)
        skill_path = os.path.join(skill_dir, "SKILL.md")
        
        with open(skill_path, "w") as f:
            f.write(f"---\nname: {skill_name}\ndescription: Test skill description\n---\n# Test Body")
            
        self.manager.scan_skills_directory()
        
        # Now delete the entire skills root directory
        shutil.rmtree(self.test_dir)
        
        # Scan again
        self.manager.scan_skills_directory()
        
        conn = sqlite3.connect(self.db_path)
        cursor = conn.cursor()
        cursor.execute("SELECT name, deleted_at FROM capabilities WHERE type = 'skill'")
        results = cursor.fetchall()
        conn.close()
        
        self.assertEqual(len(results), 1)
        self.assertIsNotNone(results[0][1]) # Should be marked as deleted

if __name__ == "__main__":
    unittest.main()

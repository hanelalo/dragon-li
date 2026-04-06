const fs = require('fs');
const file = 'src/pages/ChatPage.vue';
let content = fs.readFileSync(file, 'utf-8');
content = content.replace(
  /const res = await invoke\('session_soft_delete', { sessionId: id, deletedAt: new Date\(\)\.toISOString\(\) }\);\s*console\.log\("Delete res:", res\)/,
  `const res = await invoke('session_soft_delete', { sessionId: id, deletedAt: new Date().toISOString() });
    if (!res.ok) {
      alert("Delete failed: " + JSON.stringify(res));
      return;
    }`
);
fs.writeFileSync(file, content);

import { marked } from 'marked';
const md = `<details><summary>思考过程</summary>\n\n**bold** text\n\n</details>`;
console.log(marked.parse(md));

import DOMPurify from 'dompurify';
import { JSDOM } from 'jsdom';
const window = new JSDOM('').window;
const purify = DOMPurify(window);
const html = '<details class="reasoning-block"><summary>思考过程</summary><div>Thinking...</div></details>';
console.log(purify.sanitize(html));

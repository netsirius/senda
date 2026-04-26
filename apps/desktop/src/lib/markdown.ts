import { marked } from "marked";
import DOMPurify from "dompurify";

marked.setOptions({
  gfm: true,
  breaks: false,
});

/**
 * Render Markdown to safe HTML. We always sanitize because the prompt body
 * comes from agent files on disk that the user — or anyone with write access
 * to a connected repo — can edit. Senda treats prompt bodies as untrusted.
 */
export function renderMarkdown(input: string): string {
  const html = marked.parse(input, { async: false }) as string;
  return DOMPurify.sanitize(html);
}

import React, { useMemo, forwardRef } from "react";
import DOMPurify from "dompurify";
import type { Config } from "dompurify";

interface Props {
  html: string;
  className?: string;
  onTextSelection?: () => void;
}

const sanitizerConfig: Config = {
  ALLOWED_TAGS: [
    "b", "i", "em", "strong", "a", "p", "br", "ul", "ol", "li",
    "span", "div", "table", "thead", "tbody", "tr", "td", "th",
    "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "code", "pre",
    "hr", "img",
  ],
  ALLOWED_ATTR: ["href", "src", "alt", "title", "width", "height", "class"],
  FORBID_ATTR: ["onload", "onerror", "onclick", "onmouseover", "onfocus", "onblur"],
  FORBID_TAGS: ["script", "iframe", "object", "embed", "form", "input"],
  FORBID_CONTENTS: ["script", "style"],
};

const SanitizedPreview = forwardRef<HTMLDivElement, Props>(function SanitizedPreview(
  { html, className = "", onTextSelection },
  ref
) {
  const sanitizedHtml = useMemo(() => {
    return DOMPurify.sanitize(html, sanitizerConfig);
  }, [html]);

  return (
    <div
      ref={ref}
      className={`docx-preview select-text ${className}`}
      onMouseUp={onTextSelection}
      dangerouslySetInnerHTML={{ __html: sanitizedHtml }}
    />
  );
});

export default SanitizedPreview;

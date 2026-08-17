import React, { useMemo, forwardRef } from "react";

interface Props {
  html: string;
  className?: string;
  onTextSelection?: () => void;
}

const SanitizedPreview = forwardRef<HTMLDivElement, Props>(function SanitizedPreview(
  { html, className = "", onTextSelection },
  ref
) {
  // Enforce iframe sandboxing or sanitized container
  const sanitizedHtml = useMemo(() => {
    return html
      .replace(/<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>/gi, "")
      .replace(/\s+on\w+="[^"]*"/gi, "")
      .replace(/javascript:/gi, "disabled:");
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

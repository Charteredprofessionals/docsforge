import React, { useMemo } from "react";

interface Props {
  html: string;
  className?: string;
  onTextSelection?: () => void;
}

export default function SanitizedPreview({ html, className = "", onTextSelection }: Props) {
  // Enforce iframe sandboxing or sanitized container
  const sanitizedHtml = useMemo(() => {
    return html
      .replace(/<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>/gi, "")
      .replace(/\s+on\w+="[^"]*"/gi, "")
      .replace(/javascript:/gi, "disabled:");
  }, [html]);

  return (
    <div
      className={`docx-preview select-text ${className}`}
      onMouseUp={onTextSelection}
      dangerouslySetInnerHTML={{ __html: sanitizedHtml }}
    />
  );
}

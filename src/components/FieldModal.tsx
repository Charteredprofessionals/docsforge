import React, { useState, useEffect } from "react";
import { FieldKind, TemplateField } from "../lib/types";
import { labelToTagName } from "../lib/docxProcessor";
import { v4 as uuidv4 } from "uuid";
import { X, Tag, Check, Calendar, List, CheckSquare, PenTool, Type } from "lucide-react";

interface Props {
  selectedText: string;
  existingTags: string[];
  onSave: (field: TemplateField) => void;
  onCancel: () => void;
}

export default function FieldModal({ selectedText, existingTags, onSave, onCancel }: Props) {
  const [label, setLabel] = useState("");
  const [fieldType, setFieldType] = useState<FieldKind>("text");
  const [required, setRequired] = useState(true);
  const [optionsText, setOptionsText] = useState("Option 1, Option 2, Option 3");

  useEffect(() => {
    // Generate intelligent initial label from selected text
    const clean = selectedText.trim().replace(/^[^a-zA-Z0-9]+|[^a-zA-Z0-9]+$/g, "");
    if (clean.length > 0 && clean.length <= 40) {
      setLabel(clean);
    }
  }, [selectedText]);

  const uniqueTagName = React.useMemo(() => {
    const base = labelToTagName(label || "field");
    if (!existingTags.includes(base)) return base;
    let i = 2;
    while (existingTags.includes(`${base}_${i}`)) {
      i++;
    }
    return `${base}_${i}`;
  }, [label, existingTags]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!label.trim()) return;

    const options = fieldType === "dropdown"
      ? optionsText.split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;

    const field: TemplateField = {
      id: uuidv4(),
      label: label.trim(),
      originalText: selectedText,
      tagName: uniqueTagName,
      fieldType,
      required,
      options,
    };

    onSave(field);
  };

  const fieldTypes: { kind: FieldKind; label: string; icon: React.ElementType; desc: string }[] = [
    { kind: "text", label: "Text", icon: Type, desc: "Single-line or plain text" },
    { kind: "date", label: "Date", icon: Calendar, desc: "YYYY-MM-DD picker" },
    { kind: "dropdown", label: "Dropdown", icon: List, desc: "Select from options" },
    { kind: "checkbox", label: "Checkbox", icon: CheckSquare, desc: "True / False toggle" },
    { kind: "signature", label: "Signature", icon: PenTool, desc: "Name / Signature block" },
  ];

  return (
    <div className="fixed inset-0 bg-slate-950/80 backdrop-blur-sm flex items-center justify-center p-4 z-50 animate-in fade-in duration-150">
      <div className="bg-slate-900 border border-slate-700 rounded-2xl p-6 max-w-lg w-full shadow-2xl">
        <div className="flex items-center justify-between border-b border-slate-800 pb-4 mb-4">
          <div className="flex items-center gap-2 text-blue-400">
            <Tag className="w-5 h-5" />
            <h3 className="text-lg font-bold text-white">Create Fillable Field</h3>
          </div>
          <button onClick={onCancel} className="text-slate-400 hover:text-white transition p-1">
            <X className="w-5 h-5" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          {/* Selected Text Context */}
          <div className="bg-slate-800/60 border border-slate-700/60 rounded-xl p-3 text-xs">
            <span className="text-slate-400 font-medium block mb-1">Replacing text in document:</span>
            <p className="text-amber-300 font-mono italic truncate" title={selectedText}>
              "{selectedText}"
            </p>
          </div>

          {/* Field Label */}
          <div>
            <label className="block text-slate-300 text-sm font-medium mb-1">Field Label *</label>
            <input
              type="text"
              required
              autoFocus
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              placeholder="e.g. Employee Name, Start Date, Contract Value"
              className="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-blue-500"
            />
            {label.trim() && (
              <p className="text-xs text-slate-400 mt-1 flex items-center gap-1 font-mono">
                Placeholder tag: <span className="text-amber-400 font-semibold">{`{{${uniqueTagName}}}`}</span>
              </p>
            )}
          </div>

          {/* Field Type Selection */}
          <div>
            <label className="block text-slate-300 text-sm font-medium mb-2">Field Type</label>
            <div className="grid grid-cols-2 gap-2">
              {fieldTypes.map((t) => {
                const Icon = t.icon;
                const isSelected = fieldType === t.kind;
                return (
                  <button
                    key={t.kind}
                    type="button"
                    onClick={() => setFieldType(t.kind)}
                    className={`flex items-center gap-2.5 p-2.5 rounded-xl border text-left transition ${
                      isSelected
                        ? "bg-blue-600/20 border-blue-500 text-white"
                        : "bg-slate-800/40 border-slate-700/60 text-slate-400 hover:border-slate-600 hover:text-slate-200"
                    }`}
                  >
                    <Icon className={`w-4 h-4 shrink-0 ${isSelected ? "text-blue-400" : "text-slate-500"}`} />
                    <div className="overflow-hidden">
                      <div className="text-xs font-semibold">{t.label}</div>
                      <div className="text-[10px] text-slate-500 truncate">{t.desc}</div>
                    </div>
                  </button>
                );
              })}
            </div>
          </div>

          {/* Dropdown options if dropdown type selected */}
          {fieldType === "dropdown" && (
            <div className="bg-slate-800/40 border border-slate-700/60 rounded-xl p-3">
              <label className="block text-slate-300 text-xs font-medium mb-1">
                Dropdown Options (comma-separated)
              </label>
              <input
                type="text"
                value={optionsText}
                onChange={(e) => setOptionsText(e.target.value)}
                placeholder="Option 1, Option 2, Option 3"
                className="w-full bg-slate-800 border border-slate-700 rounded-lg px-3 py-1.5 text-white text-xs focus:outline-none focus:border-blue-500"
              />
            </div>
          )}

          {/* Required Checkbox */}
          <div className="flex items-center gap-2 pt-1">
            <input
              type="checkbox"
              id="field-required"
              checked={required}
              onChange={(e) => setRequired(e.target.checked)}
              className="rounded bg-slate-800 border-slate-700 text-blue-600 focus:ring-blue-500 w-4 h-4 cursor-pointer"
            />
            <label htmlFor="field-required" className="text-slate-300 text-xs font-medium cursor-pointer">
              Mark field as required when generating documents
            </label>
          </div>

          {/* Action Buttons */}
          <div className="flex justify-end gap-3 pt-4 border-t border-slate-800">
            <button
              type="button"
              onClick={onCancel}
              className="px-4 py-2 text-slate-400 hover:text-white text-sm font-medium transition"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={!label.trim()}
              className="flex items-center gap-2 bg-blue-600 hover:bg-blue-500 disabled:opacity-50 text-white px-5 py-2 rounded-xl text-sm font-semibold transition shadow-lg shadow-blue-600/20"
            >
              <Check className="w-4 h-4" />
              Add Field
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

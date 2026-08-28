import React, { useEffect, useState, useCallback } from "react";
import {
  renderMatterForm,
  setMatterValue,
  validateMatter,
  updateMatterStatus,
} from "../lib/ipc";
import type { MatterForm as MatterFormData, ValidationReport, FieldDef, FormGroup } from "../lib/types";
import {
  FileText,
  Save,
  CheckCircle,
  AlertCircle,
  Loader2,
  ChevronDown,
  ChevronRight,
} from "lucide-react";

interface Props {
  matterId: string;
  onComplete: () => void;
  onCancel: () => void;
}

export default function MatterForm({ matterId, onComplete, onCancel }: Props) {
  const [form, setForm] = useState<MatterFormData | null>(null);
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [validationReport, setValidationReport] = useState<ValidationReport | null>(null);
  const [saving, setSaving] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());

  useEffect(() => {
    loadForm();
  }, [matterId]);

  const loadForm = async () => {
    try {
      setLoadError(null);
      const formData = await renderMatterForm(matterId);
      setForm(formData);
      
      // Auto-expand shared groups
      const sharedGroupIds = formData.groups
        .filter((g) => g.group.scope === "shared")
        .map((g) => g.group.id);
      setExpandedGroups(new Set(sharedGroupIds));

      // Load existing values
      const initialValues: Record<string, unknown> = {};
      formData.groups.forEach((group) => {
        group.fields.forEach((field) => {
          if (field.value !== undefined) {
            initialValues[field.id] = field.value;
          }
        });
      });
      setValues(initialValues);
    } catch (e) {
      setLoadError(`Failed to load form: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  const toggleGroup = (groupId: string) => {
    setExpandedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(groupId)) {
        next.delete(groupId);
      } else {
        next.add(groupId);
      }
      return next;
    });
  };

  const handleFieldChange = useCallback(
    async (fieldId: string, value: unknown) => {
      setValues((prev) => ({ ...prev, [fieldId]: value }));
      setErrors((prev) => {
        const next = { ...prev };
        delete next[fieldId];
        return next;
      });

      // Auto-save with debounce
      setSaving(true);
      try {
        await setMatterValue(matterId, fieldId, value);
      } catch (e) {
        setErrors((prev) => ({
          ...prev,
          [fieldId]: e instanceof Error ? e.message : String(e),
        }));
      } finally {
        setSaving(false);
      }
    },
    [matterId]
  );

  const handleFieldBlur = async (fieldId: string) => {
    // Validate on blur
    try {
      const report = await validateMatter(matterId);
      setValidationReport(report);
      
      // Map errors to fields
      const fieldErrors: Record<string, string> = {};
      report.errors.forEach((err) => {
        fieldErrors[err.fieldId] = err.error;
      });
      setErrors(fieldErrors);
    } catch (e) {
      console.error("Validation failed:", e);
    }
  };

  const handleSubmit = async () => {
    setSubmitting(true);
    try {
      const report = await validateMatter(matterId);
      setValidationReport(report);

      if (!report.valid) {
        const fieldErrors: Record<string, string> = {};
        report.errors.forEach((err) => {
          fieldErrors[err.fieldId] = err.error;
        });
        setErrors(fieldErrors);
        return;
      }

      await updateMatterStatus(matterId, "ready");
      onComplete();
    } catch (e) {
      setLoadError(`Submission failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setSubmitting(false);
    }
  };

  const renderField = (field: { id: string; fieldDef: FieldDef; value?: unknown; error?: string }) => {
    const fieldDef = field.fieldDef;
    const value = values[field.id] ?? field.value ?? fieldDef.defaultValue ?? "";
    const error = errors[field.id] || field.error;

    const commonClasses = `w-full px-3 py-2 rounded-lg bg-slate-900/70 border ${
      error ? "border-red-500" : "border-slate-700"
    } text-slate-100 text-sm focus:outline-none focus:ring-2 ${
      error ? "focus:ring-red-500" : "focus:ring-blue-500"
    }`;

    const handleChange = (val: unknown) => handleFieldChange(field.id, val);
    const handleBlur = () => handleFieldBlur(field.id);

    switch (fieldDef.fieldType) {
      case "text":
      case "email":
      case "phone":
      case "url":
        return (
          <input
            type={fieldDef.fieldType === "email" ? "email" : fieldDef.fieldType === "url" ? "url" : "text"}
            value={String(value)}
            onChange={(e) => handleChange(e.target.value)}
            onBlur={handleBlur}
            placeholder={fieldDef.placeholder}
            className={commonClasses}
            required={fieldDef.required}
          />
        );

      case "multiline_text":
        return (
          <textarea
            value={String(value)}
            onChange={(e) => handleChange(e.target.value)}
            onBlur={handleBlur}
            placeholder={fieldDef.placeholder}
            rows={3}
            className={commonClasses}
            required={fieldDef.required}
          />
        );

      case "number":
      case "currency":
      case "percentage":
        return (
          <input
            type="number"
            value={String(value)}
            onChange={(e) => handleChange(e.target.value ? parseFloat(e.target.value) : "")}
            onBlur={handleBlur}
            placeholder={fieldDef.placeholder}
            step={fieldDef.fieldType === "currency" ? "0.01" : fieldDef.fieldType === "percentage" ? "0.1" : "1"}
            className={commonClasses}
            required={fieldDef.required}
          />
        );

      case "date":
        return (
          <input
            type="date"
            value={String(value)}
            onChange={(e) => handleChange(e.target.value)}
            onBlur={handleBlur}
            className={commonClasses}
            required={fieldDef.required}
          />
        );

      case "datetime":
        return (
          <input
            type="datetime-local"
            value={String(value)}
            onChange={(e) => handleChange(e.target.value)}
            onBlur={handleBlur}
            className={commonClasses}
            required={fieldDef.required}
          />
        );

      case "boolean":
        return (
          <label className="flex items-center gap-2 text-sm text-slate-300">
            <input
              type="checkbox"
              checked={Boolean(value)}
              onChange={(e) => handleChange(e.target.checked)}
              onBlur={handleBlur}
              className="accent-blue-500 w-4 h-4"
            />
            <span>{fieldDef.label}</span>
          </label>
        );

      case "select":
        return (
          <select
            value={String(value)}
            onChange={(e) => handleChange(e.target.value)}
            onBlur={handleBlur}
            className={commonClasses}
            required={fieldDef.required}
          >
            <option value="">Select...</option>
            {fieldDef.options?.map((opt) => (
              <option key={opt} value={opt}>
                {opt}
              </option>
            ))}
          </select>
        );

      case "multiselect":
        const selectedValues = Array.isArray(value) ? value : [];
        return (
          <div className="space-y-1">
            {fieldDef.options?.map((opt) => (
              <label key={opt} className="flex items-center gap-2 text-sm text-slate-300">
                <input
                  type="checkbox"
                  checked={selectedValues.includes(opt)}
                  onChange={(e) => {
                    const newValues = e.target.checked
                      ? [...selectedValues, opt]
                      : selectedValues.filter((v) => v !== opt);
                    handleChange(newValues);
                  }}
                  onBlur={handleBlur}
                  className="accent-blue-500 w-4 h-4"
                />
                <span>{opt}</span>
              </label>
            ))}
          </div>
        );

      default:
        return (
          <input
            type="text"
            value={String(value)}
            onChange={(e) => handleChange(e.target.value)}
            onBlur={handleBlur}
            className={commonClasses}
          />
        );
    }
  };

  const renderGroup = (group: FormGroup) => {
    const isShared = group.group.scope === "shared";
    const isExpanded = expandedGroups.has(group.group.id);

    return (
      <div
        key={group.group.id}
        className={`bg-slate-800/50 border rounded-2xl p-5 ${
          isShared ? "border-blue-500/30" : "border-slate-700/60"
        }`}
      >
        <button
          onClick={() => toggleGroup(group.group.id)}
          className="w-full flex items-center justify-between mb-4 text-left"
        >
          <div className="flex items-center gap-2">
            {isExpanded ? (
              <ChevronDown className="w-4 h-4 text-slate-400" />
            ) : (
              <ChevronRight className="w-4 h-4 text-slate-400" />
            )}
            <h3 className={`text-lg font-semibold ${isShared ? "text-blue-400" : "text-white"}`}>
              {group.group.label}
              {isShared && (
                <span className="ml-2 text-xs font-normal text-slate-400">(Shared Fields)</span>
              )}
            </h3>
          </div>
          <span className="text-xs text-slate-400">
            {group.fields.length} field{group.fields.length !== 1 ? "s" : ""}
          </span>
        </button>

        {isExpanded && (
          <div className="space-y-4">
            {group.fields.map((field) => (
              <div key={field.id}>
                {field.fieldDef.fieldType !== "boolean" && (
                  <label className="block text-sm font-medium text-slate-300 mb-1.5">
                    {field.fieldDef.label}
                    {field.fieldDef.required && <span className="text-red-400 ml-1">*</span>}
                  </label>
                )}
                {renderField(field)}
                {field.fieldDef.helpText && (
                  <p className="mt-1 text-xs text-slate-400">{field.fieldDef.helpText}</p>
                )}
                {errors[field.id] && (
                  <p className="mt-1 text-xs text-red-400 flex items-center gap-1">
                    <AlertCircle className="w-3 h-3" />
                    {errors[field.id]}
                  </p>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    );
  };

  const calculateProgress = () => {
    if (!form) return 0;
    const totalFields = form.groups.reduce((sum, g) => sum + g.fields.length, 0);
    const filledFields = Object.keys(values).filter((k) => values[k] !== "" && values[k] !== undefined).length;
    return totalFields > 0 ? Math.round((filledFields / totalFields) * 100) : 0;
  };

  if (loadError) {
    return (
      <div className="max-w-4xl mx-auto px-8 py-8">
        <div className="bg-red-500/15 border border-red-500/30 rounded-lg p-4 text-red-300">
          <AlertCircle className="w-5 h-5 inline mr-2" />
          {loadError}
        </div>
      </div>
    );
  }

  if (!form) {
    return (
      <div className="max-w-4xl mx-auto px-8 py-8 flex items-center justify-center">
        <Loader2 className="w-8 h-8 text-blue-400 animate-spin" />
      </div>
    );
  }

  return (
    <div className="max-w-4xl mx-auto px-8 py-8 h-full overflow-y-auto">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 bg-blue-600/20 border border-blue-500/30 rounded-xl flex items-center justify-center">
            <FileText className="w-5 h-5 text-blue-400" />
          </div>
          <div>
            <h2 className="text-2xl font-bold text-white">{form.matterName}</h2>
            <p className="text-slate-400 text-sm">
              Bundle: {form.bundleName}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {saving && (
            <span className="flex items-center gap-1 text-sm text-slate-400">
              <Loader2 className="w-3 h-3 animate-spin" />
              Saving...
            </span>
          )}
          <span className="text-sm text-slate-400">
            {calculateProgress()}% complete
          </span>
        </div>
      </div>

      {/* Progress bar */}
      <div className="mb-6 h-2 bg-slate-700 rounded-full overflow-hidden">
        <div
          className="h-full bg-blue-500 transition-all duration-300"
          style={{ width: `${calculateProgress()}%` }}
        />
      </div>

      {/* Validation Summary */}
      {validationReport && !validationReport.valid && (
        <div className="mb-6 bg-red-500/15 border border-red-500/30 rounded-lg p-4">
          <div className="flex items-center gap-2 text-red-300 font-semibold mb-2">
            <AlertCircle className="w-5 h-5" />
            Validation Errors
          </div>
          <ul className="list-disc list-inside text-sm text-red-300 space-y-1">
            {validationReport.errors.map((err, idx) => (
              <li key={idx}>
                {err.fieldName}: {err.error}
              </li>
            ))}
          </ul>
        </div>
      )}

      {validationReport && validationReport.warnings.length > 0 && (
        <div className="mb-6 bg-yellow-500/15 border border-yellow-500/30 rounded-lg p-4">
          <div className="flex items-center gap-2 text-yellow-300 font-semibold mb-2">
            <AlertCircle className="w-5 h-5" />
            Warnings
          </div>
          <ul className="list-disc list-inside text-sm text-yellow-300 space-y-1">
            {validationReport.warnings.map((warn, idx) => (
              <li key={idx}>{warn}</li>
            ))}
          </ul>
        </div>
      )}

      {/* Form Groups */}
      <div className="space-y-4 mb-6">
        {/* Shared groups first */}
        {form.groups
          .filter((g) => g.group.scope === "shared")
          .map((group) => renderGroup(group))}

        {/* Document-specific groups */}
        {form.groups
          .filter((g) => g.group.scope === "document")
          .map((group) => renderGroup(group))}
      </div>

      {/* Actions */}
      <div className="flex items-center gap-3 sticky bottom-0 bg-slate-900/95 backdrop-blur-sm border-t border-slate-700 pt-4 pb-2">
        <button
          onClick={handleSubmit}
          disabled={submitting || saving}
          className="flex items-center gap-2 px-6 py-3 rounded-lg bg-blue-600 hover:bg-blue-500 disabled:bg-slate-600 text-white font-semibold transition shadow-lg shadow-blue-600/20"
        >
          {submitting ? (
            <>
              <Loader2 className="w-4 h-4 animate-spin" />
              Validating...
            </>
          ) : (
            <>
              <CheckCircle className="w-4 h-4" />
              Submit & Preview Generation
            </>
          )}
        </button>
        <button
          onClick={onCancel}
          className="px-6 py-3 rounded-lg bg-slate-700 hover:bg-slate-600 text-white transition"
        >
          Cancel
        </button>
        {validationReport && validationReport.valid && (
          <span className="flex items-center gap-1 text-green-400 text-sm">
            <CheckCircle className="w-4 h-4" />
            All fields valid
          </span>
        )}
      </div>
    </div>
  );
}

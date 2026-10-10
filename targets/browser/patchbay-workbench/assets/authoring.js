// Native controls render catalog rules; only the canonical editor validates edits.
export function authoringBasis(value) {
  return value ? JSON.stringify([value.source_document_id, value.source_revision,
    value.checked_plot_id, value.expanded_plot_id]) : null;
}

export function configurationText(value) {
  return String(value.U64 ?? value.I64 ?? value.Bool ?? value.Text ?? value.Quantity?.source ?? value.Unit?.source ?? value.TemperatureDifference?.source ?? "");
}

export function configurationValue(value, raw) {
  if (value.U64 !== undefined || value.I64 !== undefined) {
    if (!/^-?\d+$/.test(raw)) throw new Error("An exact decimal integer is required");
    const exact = BigInt(raw);
    const encoded = exact >= BigInt(Number.MIN_SAFE_INTEGER) && exact <= BigInt(Number.MAX_SAFE_INTEGER)
      ? Number(exact) : exact.toString();
    return value.U64 !== undefined ? { U64: encoded } : { I64: encoded };
  }
  if (value.Bool !== undefined) {
    if (raw !== "true" && raw !== "false") throw new Error("A Boolean choice is required");
    return { Bool: raw === "true" };
  }
  if (value.Text !== undefined) return { Text: raw };
  if (value.Quantity) return { QuantitySource: raw };
  if (value.Unit) return { UnitSource: raw };
  if (value.TemperatureDifference) return { TemperatureDifferenceSource: raw };
  throw new Error("This value requires a structured configuration editor");
}

export function renderConfigurationFields(root, fields, apply) {
  for (const field of fields) {
    const group = document.createElement("div");
    group.dataset.applicationComponent = "plot-field";
    const label = document.createElement("label");
    label.textContent = `Configure ${field.key}`;
    const choices = field.rule?.TextOneOf?.values ?? (field.value.Bool !== undefined ? ["false", "true"] : null);
    const input = document.createElement(choices ? "select" : "input");
    input.setAttribute("aria-label", label.textContent);
    if (choices) {
      for (const choice of choices) {
        const option = document.createElement("option");
        option.value = choice;
        option.textContent = choice;
        input.append(option);
      }
    } else {
      input.type = "text";
      if (field.value.U64 !== undefined || field.value.I64 !== undefined) input.inputMode = "numeric";
    }
    input.value = configurationText(field.value);
    label.append(input);
    const help = document.createElement("small");
    help.textContent = `Canonical rule: ${JSON.stringify(field.rule)}`;
    const output = document.createElement("output");
    output.setAttribute("role", "status");
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = "Apply";
    button.onclick = async () => {
      button.disabled = true;
      try {
        await apply(field.key, configurationValue(field.value, input.value));
        output.textContent = "Checked configuration applied";
      } catch (error) {
        output.textContent = error.message;
      } finally {
        button.disabled = false;
      }
    };
    group.append(label, help, button, output);
    root.append(group);
  }
}

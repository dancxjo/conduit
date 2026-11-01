// Inputs check wire representation; only the canonical editor admits edits.
export class ConfigurationValueError extends Error {}

// Compare the server's two revision axes; a newer interaction cannot revive an older Face.
export function snapshotOrder(incoming, current) {
  if (!current) return 1;
  if (incoming.revision !== current.revision) return Math.sign(incoming.revision - current.revision);
  return Math.sign(incoming.interaction.revision - current.interaction.revision);
}

// A transport acknowledgement is not an admitted edit or Save.
export function requireAuthoringSuccess(snapshot) {
  const disposition = snapshot?.interaction?.last_disposition;
  if (disposition !== "Succeeded") throw new Error(disposition ?? "No canonical acknowledgement received");
  return snapshot;
}

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
    if (!/^-?\d+$/.test(raw)) throw new ConfigurationValueError("An exact decimal integer is required");
    const exact = BigInt(raw);
    const unsigned = value.U64 !== undefined;
    if (exact < (unsigned ? 0n : -(1n << 63n)) || exact > (unsigned ? (1n << 64n) - 1n : (1n << 63n) - 1n)) {
      throw new ConfigurationValueError(`Enter ${unsigned ? "an unsigned" : "a signed"} 64-bit whole number.`);
    }
    const encoded = exact >= BigInt(Number.MIN_SAFE_INTEGER) && exact <= BigInt(Number.MAX_SAFE_INTEGER)
      ? Number(exact) : exact.toString();
    return value.U64 !== undefined ? { U64: encoded } : { I64: encoded };
  }
  if (value.Bool !== undefined) {
    if (raw !== "true" && raw !== "false") throw new ConfigurationValueError("A Boolean choice is required");
    return { Bool: raw === "true" };
  }
  if (value.Text !== undefined) return { Text: raw };
  if (value.Quantity) return { QuantitySource: raw };
  if (value.Unit) return { UnitSource: raw };
  if (value.TemperatureDifference) return { TemperatureDifferenceSource: raw };
  throw new Error("This value requires a structured configuration editor");
}

export function configurationHelp(field) {
  const rule = field.rule;
  for (const kind of ["I64Range", "U64Range", "DurationMillis"]) {
    if (rule?.[kind]) {
      const { minimum, maximum } = rule[kind];
      return `${kind === "DurationMillis" ? "Whole milliseconds" : "Whole number"} from ${minimum} to ${maximum}.`;
    }
  }
  if (rule?.DataLabel) return `Required data name, up to ${rule.DataLabel.maximum} UTF-8 bytes, without control characters.`;
  if (rule?.TextBytes) return `Text, up to ${rule.TextBytes.maximum} UTF-8 bytes.`;
  if (rule?.TextOneOf) return "Choose one of the listed values.";
  if (rule?.QuantityRange) return "Enter a quantity with a compatible unit. Exact bounds are in the rule details.";
  if (rule === "Quantity") return "Enter a quantity with its unit.";
  if (rule === "Unit") return "Enter a unit expression.";
  if (rule === "TemperatureDifference") return "Enter a temperature difference with its unit.";
  if (field.value.Bool !== undefined) return "Choose true or false.";
  return "Checked by the canonical editor. Exact requirements are in the rule details.";
}

export function renderConfigurationFields(root, fields, apply, options = {}) {
  for (const field of fields) {
    const group = document.createElement("div");
    group.dataset.applicationComponent = "plot-field";
    const label = document.createElement("label");
    label.textContent = options.label ?? (field.rule?.DataLabel ? "Data name" : `Configure ${field.key}`);
    if (field.value.Structured !== undefined) {
      const notice = document.createElement("p");
      notice.textContent = `${field.key}: this structured value is editable in canonical source; no scalar control is available.`;
      group.append(notice);
      root.append(group);
      continue;
    }
    const choices = field.rule?.TextOneOf?.values ?? (field.value.Bool !== undefined ? ["false", "true"] : null);
    const input = document.createElement(choices ? "select" : "input");
    input.id = `authoring-field-${crypto.randomUUID()}`;
    label.htmlFor = input.id;
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
    if (field.rule?.DataLabel) {
      input.required = true;
      input.maxLength = field.rule.DataLabel.maximum;
    }
    input.value = configurationText(field.value);
    label.append(input);
    const help = document.createElement("small");
    help.id = `${input.id}-help`;
    help.textContent = options.help ?? configurationHelp(field);
    const details = document.createElement("details");
    const summary = document.createElement("summary");
    summary.textContent = "Exact rule";
    const exact = document.createElement("pre");
    exact.textContent = JSON.stringify(field.rule, null, 2);
    details.append(summary, exact);
    const output = document.createElement("output");
    output.id = `${input.id}-status`;
    output.setAttribute("role", "status");
    input.setAttribute("aria-describedby", `${help.id} ${output.id}`);
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = options.buttonText ?? "Apply";
    options.decorateButton?.(button);
    button.onclick = async () => {
      button.disabled = true;
      group.setAttribute("aria-busy", "true");
      input.removeAttribute("aria-invalid");
      output.textContent = "";
      try {
        await apply(field.key, configurationValue(field.value, input.value));
        output.textContent = options.successText ?? "Checked configuration applied";
      } catch (error) {
        if (error instanceof ConfigurationValueError) input.setAttribute("aria-invalid", "true");
        output.textContent = error.message;
      } finally {
        group.removeAttribute("aria-busy");
        button.disabled = false;
      }
    };
    group.append(label, help, button, details, output);
    root.append(group);
  }
}

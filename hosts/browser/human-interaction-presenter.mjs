const QUANTITY_KIND="value/quantity@1";

function bytesForQuantity(unitTag,value){
  const bytes=new Uint8Array(9),view=new DataView(bytes.buffer);
  bytes[0]=unitTag;
  view.setBigInt64(1,BigInt(value),true);
  return [...bytes];
}

function proposal(snapshot,interaction,payload){
  return Object.freeze({
    presentation_id:snapshot.presentation_id,
    presentation_revision:snapshot.presentation_revision,
    manifestation_id:snapshot.manifestation_id,
    contract_identity:interaction.contract_identity,
    state_identity:interaction.state_identity,
    state_revision:interaction.state_revision,
    sequence:interaction.next_sequence,
    payload,
  });
}

function valuePayload(value){return {kind:"values",values:[structuredClone(value)]};}
function quantityPayload(family,value,kind="values"){
  const typed={value_kind:QUANTITY_KIND,canonical_bytes:bytesForQuantity(family.unit_tag,value)};
  return kind==="relative"?{kind,value:typed}:{kind,values:[typed]};
}

function optionByIdentity(interaction,identity){
  return interaction.options.find(option=>option.identity===identity);
}

export class BrowserInteractionPresenter {
  #root; #submit; #pending=false; #cancelled=false; #snapshot=null;
  constructor(root,{submit}){
    if(!(root instanceof HTMLElement)||typeof submit!=="function")throw new TypeError("invalid browser Presenter boundary");
    this.#root=root;this.#submit=submit;
  }
  render(snapshot){
    this.#snapshot=structuredClone(snapshot);this.#root.replaceChildren();
    this.#root.dataset.presentationId=snapshot.presentation_id;
    this.#root.dataset.presentationRevision=String(snapshot.presentation_revision);
    for(const interaction of snapshot.interactions)this.#root.append(this.#renderInteraction(interaction));
  }
  cancelPending(){this.#cancelled=true;}
  async #emit(interaction,payload,status){
    if(this.#pending){status.textContent="Refused(QueuePressure)";status.dataset.disposition="refused";return;}
    this.#pending=true;this.#cancelled=false;
    const exact=proposal(this.#snapshot,interaction,payload);
    try{
      const result=await this.#submit(exact);
      if(this.#cancelled){status.textContent="Failed(Cancelled)";status.dataset.disposition="cancelled";return;}
      status.textContent=result.disposition;status.dataset.disposition=result.disposition;
    }catch(error){
      const code=typeof error?.code==="string"?error.code:"AdapterUnavailable";
      status.textContent=`Failed(${code})`;status.dataset.disposition="failed";
    }finally{this.#pending=false;}
  }
  #shell(interaction){
    const group=document.createElement("section"),label=document.createElement("h2"),status=document.createElement("output");
    group.dataset.interactionId=interaction.semantic_id;label.textContent=interaction.label;
    status.setAttribute("aria-live","polite");status.textContent=interaction.availability==="available"?"Ready":`Unavailable(${interaction.reason_code})`;
    group.append(label);return {group,label,status};
  }
  #renderInteraction(interaction){
    const {group,label,status}=this.#shell(interaction),family=interaction.family;
    const emit=payload=>this.#emit(interaction,payload,status);
    if(family.kind==="activate"){
      const button=document.createElement("button");button.type="button";button.textContent=interaction.action_label;button.disabled=interaction.availability!=="available";button.onclick=()=>emit({kind:"activate"});group.append(button);
    }else if(family.kind==="boolean"){
      const input=document.createElement("input");input.type="checkbox";input.checked=interaction.current.boolean;input.setAttribute("aria-label",interaction.accessibility_name);input.onchange=()=>emit(valuePayload({value_kind:"value/bool@1",canonical_bytes:[input.checked?1:0]}));group.append(input);
    }else if(family.kind==="choose-one"){
      const select=document.createElement("select");select.setAttribute("aria-label",interaction.accessibility_name);
      for(const option of interaction.options){const node=document.createElement("option");node.value=option.identity;node.textContent=option.label;node.disabled=option.availability!=="available";node.selected=interaction.current.identities.includes(option.identity);select.append(node);}
      select.onchange=()=>{const selected=optionByIdentity(interaction,select.value);if(!selected)return this.#refuse(status,"RemovedOption");if(selected.availability!=="available")return this.#refuse(status,"UnavailableOption");return emit(valuePayload(selected.value));};group.append(select);
    }else if(family.kind==="choose-many"){
      const controls=document.createElement("fieldset"),legend=document.createElement("legend");legend.textContent=interaction.accessibility_name;controls.append(legend);
      for(const option of interaction.options){const row=document.createElement("label"),input=document.createElement("input");input.type="checkbox";input.dataset.optionIdentity=option.identity;input.checked=interaction.current.identities.includes(option.identity);input.disabled=option.availability!=="available";row.append(input,option.label);controls.append(row);}
      const submit=document.createElement("button");submit.type="button";submit.textContent=interaction.submit_label;submit.onclick=()=>{const ids=[...controls.querySelectorAll("input:checked")].map(node=>node.dataset.optionIdentity);if(ids.length<family.minimum_selections||ids.length>family.maximum_selections)return this.#refuse(status,"InvalidCardinality");const options=ids.map(id=>optionByIdentity(interaction,id));if(options.some(option=>!option))return this.#refuse(status,"RemovedOption");if(options.some(option=>option.availability!=="available"))return this.#refuse(status,"UnavailableOption");return emit({kind:"values",values:structuredClone(options.map(option=>option.value))});};controls.append(submit);group.append(controls);
    }else if(family.kind==="scalar"){
      const input=document.createElement("input");input.type=interaction.manifestation?.scalar==="number"?"number":"range";input.min=String(family.minimum);input.max=String(family.maximum);input.step=String(family.granularity);input.value=String(interaction.current.quantity);input.setAttribute("aria-label",interaction.accessibility_name);input.onchange=()=>{const value=Number(input.value);if(!Number.isSafeInteger(value)||value<family.minimum||value>family.maximum)return this.#refuse(status,"OutOfRange");if((value-family.minimum)%family.granularity!==0)return this.#refuse(status,"UnsupportedGranularity");return emit(quantityPayload(family,value));};group.append(input);
    }else if(family.kind==="relative"){
      for(const [text,delta] of [["Decrease",-family.granularity],["Increase",family.granularity]]){const button=document.createElement("button");button.type="button";button.textContent=text;button.onclick=()=>emit(quantityPayload(family,delta,"relative"));group.append(button);}
    }else if(family.kind==="text"){
      const input=document.createElement("input");input.type="text";input.maxLength=family.maximum_bytes;input.setAttribute("aria-label",interaction.accessibility_name);const submit=document.createElement("button");submit.type="button";submit.textContent=interaction.submit_label;submit.onclick=()=>{const bytes=[...new TextEncoder().encode(input.value)];if(bytes.length>family.maximum_bytes)return this.#refuse(status,"OversizeValue");if(!family.allow_empty&&bytes.length===0)return this.#refuse(status,"EmptyValue");return emit(valuePayload({value_kind:"value/text@1",canonical_bytes:bytes}));};group.append(input,submit);
    }else throw new TypeError(`unsupported portable interaction family ${family.kind}`);
    group.append(status);return group;
  }
  #refuse(status,code){status.textContent=`Refused(${code})`;status.dataset.disposition="refused";}
}

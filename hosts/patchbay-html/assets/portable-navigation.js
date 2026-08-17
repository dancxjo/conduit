const depthOrder = new Map([["Primary",0],["Context",1],["Detail",2],["Exact",3]]);

function itemValue(item, name) {
  return item && Object.prototype.hasOwnProperty.call(item, name) ? item[name] : undefined;
}

export function projectCurrent(snapshot) {
  const presentation = snapshot.presentation;
  const bundle = snapshot.navigation;
  if (!bundle) return { ...presentation, portable:false };
  const navigation = bundle.navigation, projection = bundle.projection, cursor = bundle.cursor;
  if (navigation.presentation !== presentation.identity || projection.presentation !== presentation.identity || cursor.presentation !== presentation.identity || navigation.revision !== presentation.revision || projection.revision !== presentation.revision || cursor.revision !== presentation.revision || projection.navigation !== navigation.identity || cursor.navigation !== navigation.identity) throw new Error("stale portable navigation identity");
  const place = navigation.places.find(candidate => candidate.place === cursor.place);
  const aspect = place?.aspects.find(candidate => candidate.aspect === cursor.aspect);
  if (!place || !aspect || !depthOrder.has(cursor.depth) || (cursor.focus !== null && !aspect.focusable_subjects.includes(cursor.focus))) throw new Error("invalid portable navigation cursor");
  const memberships = projection.memberships.filter(membership => membership.place === cursor.place && membership.aspect === cursor.aspect && depthOrder.get(membership.depth) <= depthOrder.get(cursor.depth));
  const subjects = new Set(), relationships = new Set(), properties = new Set(), text = new Set(), actions = new Set();
  for (const membership of memberships) {
    const item = membership.item;
    const subject = itemValue(item,"Subject"), relationship = itemValue(item,"Relationship"), property = itemValue(item,"Property"), line = itemValue(item,"Text"), action = itemValue(item,"Action");
    if (typeof subject === "string") subjects.add(subject);
    else if (Number.isSafeInteger(relationship) && presentation.relationships[relationship]) relationships.add(relationship);
    else if (Number.isSafeInteger(property) && presentation.properties[property]) properties.add(property);
    else if (Number.isSafeInteger(line) && presentation.text[line]) text.add(line);
    else if (typeof action === "string" && presentation.actions.some(candidate => candidate.identity === action)) actions.add(action);
    else throw new Error("invalid portable projection item");
  }
  return {
    ...presentation,
    portable:true,
    cursor,
    places:navigation.places,
    follows:navigation.follows,
    subjects:presentation.subjects.filter(subject => subjects.has(subject.identity)),
    relationships:presentation.relationships.filter((_,index) => relationships.has(index)),
    properties:presentation.properties.filter((_,index) => properties.has(index)),
    text:presentation.text.filter((_,index) => text.has(index)),
    actions:presentation.actions.filter(action => actions.has(action.identity)),
  };
}

export function lensForCursor(cursor) {
  if (!cursor || cursor.aspect === "Structure") return cursor?.place === "Program" ? "form" : "world";
  return ({Plan:"plan",Play:"play",Signs:"signs"})[cursor.aspect] ?? "world";
}

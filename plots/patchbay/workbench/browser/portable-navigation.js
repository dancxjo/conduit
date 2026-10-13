const depthOrder = new Map([["Primary",0],["Context",1],["Detail",2],["Exact",3]]);

function itemValue(item, name) {
  return item && Object.prototype.hasOwnProperty.call(item, name) ? item[name] : undefined;
}

export function projectCurrent(snapshot) {
  const face = snapshot.face;
  const bundle = snapshot.navigation;
  if (!bundle) return { ...face, portable:false };
  const navigation = bundle.navigation, projection = bundle.projection, cursor = bundle.cursor;
  if (navigation.face !== face.identity || projection.face !== face.identity || cursor.face !== face.identity || navigation.revision !== face.revision || projection.revision !== face.revision || cursor.revision !== face.revision || projection.navigation !== navigation.identity || cursor.navigation !== navigation.identity) throw new Error("stale portable navigation identity");
  const place = navigation.places.find(candidate => candidate.place === cursor.place);
  const aspect = place?.aspects.find(candidate => candidate.aspect === cursor.aspect);
  if (!place || !aspect || !depthOrder.has(cursor.depth) || (cursor.focus !== null && !aspect.focusable_subjects.includes(cursor.focus))) throw new Error("invalid portable navigation cursor");
  const memberships = projection.memberships.filter(membership => membership.place === cursor.place && membership.aspect === cursor.aspect && depthOrder.get(membership.depth) <= depthOrder.get(cursor.depth));
  const subjects = new Set(), relationships = new Set(), properties = new Set(), text = new Set(), actions = new Set();
  for (const membership of memberships) {
    const item = membership.item;
    const subject = itemValue(item,"Subject"), relationship = itemValue(item,"Relationship"), property = itemValue(item,"Property"), line = itemValue(item,"Text"), action = itemValue(item,"Action");
    if (typeof subject === "string") subjects.add(subject);
    else if (Number.isSafeInteger(relationship) && face.relationships[relationship]) relationships.add(relationship);
    else if (Number.isSafeInteger(property) && face.properties[property]) properties.add(property);
    else if (Number.isSafeInteger(line) && face.text[line]) text.add(line);
    else if (typeof action === "string" && face.actions.some(candidate => candidate.identity === action)) actions.add(action);
    else throw new Error("invalid portable projection item");
  }
  return {
    ...face,
    portable:true,
    cursor,
    places:navigation.places,
    follows:navigation.follows,
    subjects:face.subjects.filter(subject => subjects.has(subject.identity)),
    relationships:face.relationships.filter((_,index) => relationships.has(index)),
    properties:face.properties.filter((_,index) => properties.has(index)),
    text:face.text.filter((_,index) => text.has(index)),
    actions:face.actions.filter(action => actions.has(action.identity)),
  };
}

export function observeCurrent(snapshot) {
  const projected=projectCurrent(snapshot), bundle=snapshot.navigation;
  if (!projected.portable || !bundle) throw new Error("portable navigation observation unavailable");
  const place=bundle.navigation.places.find(candidate=>candidate.place===bundle.cursor.place);
  const byIdentity=(left,right)=>left.identity<right.identity?-1:left.identity>right.identity?1:0;
  return {
    schema:"conduit.face/navigation-observation@1",
    face_id:snapshot.face.identity,
    face_revision:snapshot.face.revision,
    navigation_id:bundle.navigation.identity,
    projection_id:bundle.projection.identity,
    cursor:bundle.cursor,
    available_places:bundle.navigation.places,
    available_aspects:place.aspects,
    projected_subjects:[...projected.subjects].sort(byIdentity),
    projected_actions:[...projected.actions].sort(byIdentity),
    current_follows:bundle.navigation.follows
      .filter(follow=>follow.source_subject===bundle.cursor.focus)
      .sort(byIdentity),
  };
}

export function lensForCursor(cursor) {
  if (!cursor || cursor.aspect === "Structure") return cursor?.place === "Program" ? "plot" : "world";
  return ({Plan:"plan",Play:"play",Signs:"signs"})[cursor.aspect] ?? "world";
}

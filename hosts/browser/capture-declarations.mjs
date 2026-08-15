import { lstat, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";

const SCHEMA="conduit.capture-declarations/v1";
const MAX_OUTPUTS=64;
const MAX_DOCUMENT_BYTES=1024*1024;

function validateDocument(document) {
  if(document===null||typeof document!=="object"||document.schema!==SCHEMA||!Array.isArray(document.outputs)){
    throw new Error(`capture declarations must use ${SCHEMA}`);
  }
  if(document.outputs.length>MAX_OUTPUTS)throw new Error(`capture declarations exceed bounded maximum ${MAX_OUTPUTS}`);
  const identities=new Set();
  const paths=new Set();
  for(const output of document.outputs){
    if(output===null||typeof output!=="object"||typeof output.id!=="string"||!output.id||typeof output.path!=="string"||!output.path){
      throw new Error("capture declaration identities and paths must be non-empty strings");
    }
    if(identities.has(output.id))throw new Error(`duplicate capture identity '${output.id}'`);
    if(paths.has(output.path))throw new Error(`duplicate capture path '${output.path}'`);
    identities.add(output.id);
    paths.add(output.path);
  }
  return document.outputs;
}

async function readExisting(file) {
  try {
    const metadata=await lstat(file);
    if(!metadata.isFile()||metadata.size>MAX_DOCUMENT_BYTES){
      throw new Error(`capture declarations must be one regular file no larger than ${MAX_DOCUMENT_BYTES} bytes`);
    }
    return validateDocument(JSON.parse(await readFile(file,"utf8")));
  } catch(error) {
    if(error?.code==="ENOENT")return [];
    throw error;
  }
}

export async function persistCaptureDeclaration(evidenceRoot,output) {
  validateDocument({schema:SCHEMA,outputs:[output]});
  const file=path.join(evidenceRoot,"captures.json");
  const outputs=await readExisting(file);
  const sameId=outputs.findIndex(existing=>existing.id===output.id);
  const samePath=outputs.findIndex(existing=>existing.path===output.path);
  if((sameId>=0||samePath>=0)&&sameId!==samePath){
    throw new Error(`capture declaration conflicts for identity '${output.id}' and path '${output.path}'`);
  }
  if(sameId>=0)outputs[sameId]=output;else outputs.push(output);
  validateDocument({schema:SCHEMA,outputs});
  const document=JSON.stringify({schema:SCHEMA,outputs},null,2);
  if(Buffer.byteLength(document,"utf8")+1>MAX_DOCUMENT_BYTES){
    throw new Error(`capture declarations exceed bounded maximum ${MAX_DOCUMENT_BYTES} bytes`);
  }
  const temporary=path.join(evidenceRoot,"captures.json.tmp");
  await writeFile(temporary,`${document}\n`,{encoding:"utf8",flag:"wx"});
  await rename(temporary,file);
}

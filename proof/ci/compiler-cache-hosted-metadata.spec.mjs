import test from 'node:test';
import assert from 'node:assert/strict';
import {pathToFileURL} from 'node:url';
import {canonicalHostedMetadata} from '../../tools/ci/compiler-cache/hosted-metadata.mjs';
function graph(root,home){
 const sourceRoot=root+'/source',gitCheckout=home+'/git/checkouts/tongues/random/b5d7535';
 const id='path+'+pathToFileURL(sourceRoot+'/products/conduit').href+'#0.1.0';
 const registry=home+'/registry/src/random/serde-1.0.0';
 const metadata={version:1,workspace_root:sourceRoot,target_directory:sourceRoot+'/target',build_directory:sourceRoot+'/target',workspace_members:[id],packages:[
 {name:'conduit',version:'0.1.0',id,source:null,manifest_path:sourceRoot+'/products/conduit/Cargo.toml',targets:[{src_path:sourceRoot+'/products/conduit/src/main.rs'}],dependencies:[{name:'local',path:sourceRoot+'/local'}],metadata:{literal:'unchanged'}},
 {name:'serde',version:'1.0.0',id:'registry+https://github.com/rust-lang/crates.io-index#serde@1.0.0',source:'registry+https://github.com/rust-lang/crates.io-index',manifest_path:registry+'/Cargo.toml',targets:[{src_path:registry+'/src/lib.rs'}]},
 {name:'speaking',version:'0.1.0',source:'git+https://github.com/dancxjo/tongues.git?rev=b5d7535#b5d7535',manifest_path:gitCheckout+'/crates/speaking/Cargo.toml',targets:[{src_path:gitCheckout+'/crates/speaking/src/lib.rs'}]}
 ],resolve:{root:id,nodes:[{id,deps:[{pkg:id}],dependencies:[id],features:['default']}]}};
 return {metadata,roots:{sourceRoot,gitCheckout}};
}
test('same complete Cargo graph from different acquisition and Cargo homes has identical frozen metadata',()=>{
 const a=graph('/tmp/first runner#literal?','/home/a/.cargo'),b=graph('/runner/temp/second','/home/b/cargo');
 const first=canonicalHostedMetadata(a.metadata,a.roots),second=canonicalHostedMetadata(b.metadata,b.roots);
 assert.deepEqual(first,second);
 const text=JSON.stringify(first);assert.ok(!text.includes('/tmp/')&&!text.includes('/home/'));
 assert.equal(first.target_directory,'/task/target');assert.equal(first.workspace_root,'/source');
 assert.equal(first.packages[1].manifest_path,'/vendor/serde-1.0.0/Cargo.toml');
 assert.equal(first.packages[2].targets[0].src_path,'/git-vendor/tongues/crates/speaking/src/lib.rs');
 assert.equal(first.resolve.nodes[0].id,first.workspace_members[0]);
});
test('real source graph and feature differences remain namespace-significant',()=>{
 const a=graph('/tmp/a','/home/a'),baseline=canonicalHostedMetadata(a.metadata,a.roots);
 a.metadata.resolve.nodes[0].features.push('different');assert.notDeepEqual(canonicalHostedMetadata(a.metadata,a.roots),baseline);
 a.metadata.resolve.nodes[0].features.pop();a.metadata.packages[0].metadata.literal='changed source metadata';assert.notDeepEqual(canonicalHostedMetadata(a.metadata,a.roots),baseline);
});
test('unknown external, traversal and unsupported registry acquisition paths refuse',()=>{
 for(const bad of ['/private/unreviewed.rs','/tmp/a/source/../outside.rs']){
 const a=graph('/tmp/a','/home/a');a.metadata.packages[0].targets[0].src_path=bad;
 assert.throws(()=>canonicalHostedMetadata(a.metadata,a.roots),{code:'unsupported-hosted-metadata-path'});
 }
 const a=graph('/tmp/a','/home/a');a.metadata.packages[1].source='registry+https://other.invalid';
 assert.throws(()=>canonicalHostedMetadata(a.metadata,a.roots),{code:'unsupported-hosted-metadata-path'});
});

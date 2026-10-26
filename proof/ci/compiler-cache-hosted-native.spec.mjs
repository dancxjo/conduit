import test from 'node:test';
import assert from 'node:assert/strict';
import {nativeGccSelection,nativeRuntimePath,parseNativeDependencies} from '../../tools/ci/compiler-cache/hosted-native.mjs';
test('native selection supports actual Ubuntu GCC13 and Debian GCC15 private compiler layouts',()=>{
 const select=major=>({target:'x86_64-linux-gnu',major,libgcc:`/usr/lib/gcc/x86_64-linux-gnu/${major}/libgcc.a`,cc1:`/usr/${major==='13'?'lib':'libexec'}/gcc/x86_64-linux-gnu/${major}/cc1`,collect2:`/usr/${major==='13'?'lib':'libexec'}/gcc/x86_64-linux-gnu/${major}/collect2`});
 assert.deepEqual(nativeGccSelection(select('13')).trees,['/usr/lib/gcc/x86_64-linux-gnu/13']);
 assert.deepEqual(nativeGccSelection(select('15')).trees,['/usr/lib/gcc/x86_64-linux-gnu/15','/usr/libexec/gcc/x86_64-linux-gnu/15']);
 assert.throws(()=>nativeGccSelection({...select('13'),target:'aarch64-linux-gnu'}),{code:'unsupported-hosted-native-toolchain'});
 assert.throws(()=>nativeGccSelection({...select('13'),cc1:'/tmp/ambient/cc1'}),{code:'unsupported-hosted-native-toolchain'});
});
test('actual loader output chooses the installed soname version and preserves interpreter lookup',()=>{
 assert.deepEqual(parseNativeDependencies('linux-vdso.so.1 (0x123)\nlibbfd-2.42-system.so => /lib/x86_64-linux-gnu/libbfd-2.42-system.so (0xabcd)\n/lib64/ld-linux-x86-64.so.2 (0x12)\n'),['/lib/x86_64-linux-gnu/libbfd-2.42-system.so','/lib64/ld-linux-x86-64.so.2']);
 assert.equal(nativeRuntimePath('/lib/x86_64-linux-gnu/libc.so.6'),'usr/lib/x86_64-linux-gnu/libc.so.6');
 assert.equal(nativeRuntimePath('/lib64/ld-linux-x86-64.so.2'),'usr/lib64/ld-linux-x86-64.so.2');
 assert.throws(()=>parseNativeDependencies('libbfd.so => not found'),{code:'unsupported-hosted-native-dependency'});
 assert.throws(()=>nativeRuntimePath('/home/private/unknown.so'),{code:'unsupported-hosted-native-path'});
});

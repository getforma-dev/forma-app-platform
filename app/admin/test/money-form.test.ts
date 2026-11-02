import assert from 'node:assert/strict';
import { test } from 'node:test';
import { moneyMinorUnits, typedFormValue } from '../src/canvas/commands.ts';
const spec = {type:'money',currency:'USD',minor_unit_exponent:2};
test('money forms convert human decimal text to exact integer minor units without float',()=>{
 for(const [display,minor] of [['12.50','1250'],['12','1200'],['.50','50'],['0.01','1'],['-12.50','-1250'],['-0.00','0'],['00012.50','1250'],[' 12.50 ','1250'],['999999999999999999999999999999999999.99','99999999999999999999999999999999999999']]) {
  assert.equal(moneyMinorUnits(display,2),minor);
  assert.deepEqual(typedFormValue(spec,display),{type:'money',currency:'USD',minor_unit_exponent:2,minor_units:minor});
 }
});
test('money forms reject overprecision, exponent notation, punctuation and overflow',()=>{
 for(const value of ['1e2','12.501','1,250.00','$12.50','NaN','+1.00','--1','1.2.3','.','9999999999999999999999999999999999999.99'])assert.equal(typedFormValue(spec,value),null,value);
 assert.equal(moneyMinorUnits('2.5',1),'25');assert.equal(moneyMinorUnits('2.5',0),null);
 for(const exponent of [-1,99,1.5,'2',null])assert.equal(moneyMinorUnits('1',exponent),null);
});
test('money conversion leaves nullable and legacy integer/decimal form semantics intact',()=>{
 assert.deepEqual(typedFormValue(spec,''),{type:'null'});
 assert.deepEqual(typedFormValue({type:'integer',unit:'items'},'1250'),{type:'integer',unit:'items',value:'1250'});
 assert.deepEqual(typedFormValue({type:'decimal',scale:2,unit:'usd'},'1250'),{type:'decimal',scale:2,unit:'usd',coefficient:'1250'});
});

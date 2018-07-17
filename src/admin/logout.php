<?php
include_once 'includes/controller.php';

/* 
 * Use this page to log your users out of your site. After logout the user is redirected as requried.
 * You can use the GET method to alternate between the admin home page - logout.php?path=admin - and 
 * the login page - logout.php OR create your own. See commented out example below.
 * 
 */

if(isset($_GET['path'])) {
    if ($_GET['path'] == 'admin'){
        $path = $configs->homePage();
        logout($session, $path);
    } else if($_GET['path'] == 'referrer'){
        $path = $session->referrer;
        logout($session, $path);
    }
    //else if($_GET['path'] == 'example') {
    //    $path = 'http://www.example.com';
    //    logout($session, $path);
    //}
    
/* No path specified - go to login page specified in database */    
} else {
    $path = $configs->loginPage();
    logout($session, $path);
}

function logout($session, $path) {
    $session->logout();
    header("Location: " . $path);
}
?>